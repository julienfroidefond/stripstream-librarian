"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";

import { Button, Modal, toast } from "@/app/components/ui";
import { browserErrorMessage } from "@/lib/api/browser";
import { ApiError } from "@/lib/api/errors";
import {
  createDownloadActionStore,
  isStaleReleaseError,
  needsConfirmation,
  type DownloadAction,
  type DownloadActionContext,
  type DownloadItemActionState,
  type DownloadActionStore,
} from "@/lib/downloads/actions";
import { useTranslation } from "@/lib/i18n/context";

/** Upstream message for a failed action, preserving `Error N` for HTTP errors. */
export function browserFailureMessage(error: unknown): string {
  if (error instanceof ApiError) return browserErrorMessage(error, `Error ${error.status}`);
  return error instanceof Error ? error.message : String(error);
}

/** Copy of the confirmation dialog; the wording belongs to the row. */
export interface DownloadConfirmPrompt {
  title: string;
  message: string;
  confirmLabel: string;
}

export interface DownloadRunOptions {
  context?: DownloadActionContext;
  /**
   * Ask first, with this copy. Mandatory for every route whose
   * `needsConfirmation` is true; also accepted for a route that does not ask by
   * default, when the verb is destructive *in that row's context*.
   */
  confirm?: DownloadConfirmPrompt;
}

/** What the rows consume: one `busy` read and one fire-and-forget `run`. */
export interface DownloadActions {
  /** Whether the item has a request in flight. */
  busy(id: string): boolean;
  /** The item's state, so a row can pick the right spinner. */
  state(id: string): DownloadItemActionState;
  /**
   * Run one action. Confirmation is resolved by the shared dialog, and the
   * store coalesces a burst of clicks on the same item; the returned nothing is
   * deliberate — the store never rejects (failures reach the toast).
   */
  run(action: DownloadAction, id: string, options?: DownloadRunOptions): void;
}

/** Internal surface the provider exposes so the hook can subscribe. */
interface DownloadActionsStore extends DownloadActions {
  subscribe(listener: () => void): () => void;
  version(): number;
}

interface PendingConfirmation {
  action: DownloadAction;
  id: string;
  context?: DownloadActionContext;
  prompt?: DownloadConfirmPrompt;
}

const SERVER_VERSION = () => 0;

const NO_SUBSCRIBE = () => () => {};

const DownloadActionsContext = createContext<DownloadActionsStore | null>(null);

/**
 * One download write seam for the downloads page.
 *
 * It owns the store, the single confirmation dialog and the wiring to the
 * page's refresh callback and to the global toast. Rows read `busy`/`state` and
 * call `run`; they never touch `browserApi` for a retry, a start, a delete or a
 * dismiss, and never hold a `retrying`/`deleting` flag of their own.
 *
 * Failure policy (from the store): the row resets, the toast shows the upstream
 * message and nothing is refreshed. Success resets the row and calls
 * `onRefresh` — the page decides what refreshing means for that action.
 */
export function DownloadActionsProvider({
  onRefresh,
  children,
}: {
  /** Called after each successful action, with the action and the item id. */
  onRefresh: (action: DownloadAction, id: string) => void;
  children: ReactNode;
}) {
  const { t } = useTranslation();

  // The store is created once and outlives every render, so it reads the
  // callbacks through refs instead of capturing the first ones.
  const refreshRef = useRef(onRefresh);
  const tRef = useRef(t);
  useEffect(() => {
    refreshRef.current = onRefresh;
    tRef.current = t;
  }, [onRefresh, t]);

  const [store] = useState<DownloadActionStore>(() =>
    createDownloadActionStore({
      onRefresh: (action, id) => refreshRef.current(action, id),
      onError: (error, action, id) => {
        // One exception to "no refresh on failure": a stale release index
        // (409) means the list on screen is outdated; nothing was removed.
        if (isStaleReleaseError(error, action)) {
          toast(tRef.current("downloads.releaseChanged"), "error");
          refreshRef.current(action, id);
          return;
        }
        toast(browserFailureMessage(error), "error");
      },
    }),
  );
  useEffect(() => () => store.dispose(), [store]);

  const [pending, setPending] = useState<PendingConfirmation | null>(null);
  // Bumped on every store notification: `useSyncExternalStore` compares this
  // snapshot to decide that a row's `busy`/`state` read is stale.
  const versionRef = useRef(0);

  const [actions] = useState<DownloadActionsStore>(() => ({
    subscribe(listener) {
      return store.subscribe(() => {
        versionRef.current += 1;
        listener();
      });
    },
    version: () => versionRef.current,
    busy: (id) => store.busy(id),
    state: (id) => store.getState(id),
    run(action, id, options = {}) {
      if (!options.confirm && !needsConfirmation(action.target, action.verb)) {
        void store.run(action, id, options.context);
        return;
      }
      setPending({ action, id, context: options.context, prompt: options.confirm });
    },
  }));

  const cancel = useCallback(() => setPending(null), []);
  const accept = useCallback(() => {
    if (!pending) return;
    setPending(null);
    void store.run(pending.action, pending.id, pending.context);
  }, [pending, store]);

  return (
    <DownloadActionsContext.Provider value={actions}>
      {children}
      <Modal
        isOpen={pending !== null}
        onClose={cancel}
        maxWidth="sm"
        title={pending?.prompt?.title ?? t("downloads.delete")}
        footer={
          <>
            <Button variant="outline" size="sm" onClick={cancel}>
              {t("common.cancel")}
            </Button>
            <Button variant="destructive" size="sm" onClick={accept}>
              {pending?.prompt?.confirmLabel ?? t("common.delete")}
            </Button>
          </>
        }
      >
        <p className="text-sm text-muted-foreground">
          {pending?.prompt?.message ?? t("downloads.confirmDelete")}
        </p>
      </Modal>
    </DownloadActionsContext.Provider>
  );
}

/**
 * The page's download actions. Subscribing here is what makes `busy(id)` and
 * `state(id)` re-render their caller: both read the store synchronously.
 */
export function useDownloadActions(): DownloadActions {
  const actions = useContext(DownloadActionsContext);
  // Hooks stay unconditional: a row mounted outside the provider must still
  // fail with the message below, not with a hooks-order error.
  const version = useSyncExternalStore(
    actions?.subscribe ?? NO_SUBSCRIBE,
    actions?.version ?? SERVER_VERSION,
    SERVER_VERSION,
  );
  const resolved = useMemo(() => actions, [actions, version]);
  if (!resolved) {
    throw new Error("useDownloadActions must be used inside <DownloadActionsProvider>");
  }
  return resolved;
}
