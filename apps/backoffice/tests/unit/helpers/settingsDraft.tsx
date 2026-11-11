import { render, type RenderOptions, type RenderResult } from "@testing-library/react";
import type { ReactElement, ReactNode } from "react";
import { vi, type Mock } from "vitest";

import {
  SettingsDraftProvider,
  type SettingsSectionWriter,
} from "@/app/(app)/settings/SettingsDraftProvider";

export interface RenderWithSettingsOptions extends Omit<RenderOptions, "wrapper"> {
  /** Stored `app_settings` values; the provider seeds every known default. */
  initial?: Record<string, unknown>;
  /** Replace the proxy write. Defaults to acknowledging the submitted value. */
  writer?: SettingsSectionWriter;
}

export interface SettingsRenderResult extends RenderResult {
  writer: Mock<SettingsSectionWriter>;
}

/**
 * Render a settings card inside the draft provider with a controllable writer.
 *
 * A card no longer receives `handleUpdateSetting`/`initialData` props: it reads
 * its slice from the provider, and every write goes through `writer`.
 */
export function renderWithSettingsDraft(
  ui: ReactElement,
  { initial = {}, writer, ...options }: RenderWithSettingsOptions = {},
): SettingsRenderResult {
  const writerMock = vi.fn<SettingsSectionWriter>(
    writer ?? ((_key, value) => Promise.resolve(value)),
  );
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <SettingsDraftProvider initial={initial} writer={writerMock}>
      {children}
    </SettingsDraftProvider>
  );

  return {
    ...render(ui, { wrapper: Wrapper, ...options }),
    writer: writerMock,
  };
}
