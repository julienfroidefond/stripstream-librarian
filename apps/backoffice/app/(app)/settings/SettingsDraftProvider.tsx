"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { toast } from "@/app/components/ui";
import { browserApi } from "@/lib/api/browser";
import { useTranslation } from "@/lib/i18n/context";
import type { TranslationKey } from "@/lib/i18n/fr";
import {
  createSettingsDraft,
  type SettingsDraft,
  type SettingsDraftSaveResult,
} from "@/lib/settingsDraft";
import { seedSettingsSections, validateSettingsSection } from "@/lib/settingsSections";

/**
 * Browser write for one `app_settings` section.
 *
 * `apiFetch` is server-only (it needs the bootstrap token), so the browser goes
 * through the Next.js proxy via `browserApi.settings.saveSection`. A non-2xx
 * response becomes a rejected promise; the draft then rolls the section back to
 * its acknowledged value. A successful response returns the stored section (the
 * API may redact a secret), which the draft adopts as its new baseline.
 */
export async function writeSettingsSection(key: string, value: unknown): Promise<unknown> {
  return browserApi.settings.saveSection(key, value);
}

export type SettingsSectionWriter = (key: string, value: unknown) => Promise<unknown>;

const SettingsDraftContext = createContext<SettingsDraft | null>(null);

/**
 * Owns the one settings draft for the settings route and reports every write
 * result through the global toast. Cards read their slice with
 * `useSettingsSection` and never call `fetch("/api/settings/...")` themselves.
 */
export function SettingsDraftProvider({
  initial,
  children,
  writer = writeSettingsSection,
}: {
  /** Values loaded by the server page; defaults are merged here. */
  initial: Record<string, unknown>;
  children: ReactNode;
  /** Test seam: replace the proxy write without mounting the whole page. */
  writer?: SettingsSectionWriter;
}) {
  const { t } = useTranslation();
  const [draft] = useState<SettingsDraft>(() =>
    createSettingsDraft({
      initial: seedSettingsSections(initial),
      write: writer,
      validate: validateSettingsSection,
      onResult: (key, result) => {
        if (result.status === "saved") {
          toast(t("settings.savedSuccess"), "success");
        } else if (result.status === "invalid") {
          toast(t(result.message as TranslationKey), "error");
        } else if (result.status === "error") {
          // Surface the upstream validation message (for example a rejected
          // cache directory) instead of a generic failure.
          toast(`${t("settings.savedError")}: ${result.message}`, "error");
        }
      },
    }),
  );

  useEffect(() => () => draft.dispose(), [draft]);

  return <SettingsDraftContext.Provider value={draft}>{children}</SettingsDraftContext.Provider>;
}

export function useSettingsDraft(): SettingsDraft {
  const draft = useContext(SettingsDraftContext);
  if (!draft) {
    throw new Error("useSettingsDraft must be used inside <SettingsDraftProvider>");
  }
  return draft;
}

export interface SettingsSectionHandle<T> {
  value: T;
  set(value: T): void;
  patch(patch: Partial<T>): void;
  save(): Promise<SettingsDraftSaveResult>;
  reset(): void;
  dirty: boolean;
  saving: boolean;
  error: string | undefined;
}

/**
 * Subscribe to one section. The snapshot object is cached by the draft, so a
 * card only re-renders when its own section changes.
 */
export function useSettingsSection<T>(key: string): SettingsSectionHandle<T> {
  const draft = useSettingsDraft();
  const subscribe = useCallback(
    (onStoreChange: () => void) => draft.subscribe(onStoreChange),
    [draft],
  );
  const getSnapshot = useCallback(() => draft.getSectionSnapshot<T>(key), [draft, key]);
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  const set = useCallback((value: T) => draft.set(key, value), [draft, key]);
  const patch = useCallback(
    (value: Partial<T>) => draft.patch(key, value as Record<string, unknown>),
    [draft, key],
  );
  const save = useCallback(() => draft.save(key), [draft, key]);
  const reset = useCallback(() => draft.reset(key), [draft, key]);

  return {
    // The provider seeds every known key, so the value is always present.
    value: snapshot.value as T,
    set,
    patch,
    save,
    reset,
    dirty: snapshot.dirty,
    saving: snapshot.saving,
    error: snapshot.error,
  };
}

/** True while any section has a request in flight (tab-level saving badge). */
export function useSettingsSaving(): boolean {
  const draft = useSettingsDraft();
  const subscribe = useCallback(
    (onStoreChange: () => void) => draft.subscribe(onStoreChange),
    [draft],
  );
  const getSnapshot = useCallback(() => draft.getSnapshot().savingKeys.length > 0, [draft]);
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
