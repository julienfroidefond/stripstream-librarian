/**
 * Write seam for the acquisition downloads rows (retry / start / delete / dismiss).
 *
 * The downloads page drives six targets: torrents imported through the active
 * client, AllDebrid magnets, direct-download (DDL) provider downloads, the
 * Telegram monitor, one release of an "available downloads" entry and the
 * detection entries of an available group. Each target exposes a different
 * subset of the same verbs, through a different endpoint, and the page
 * previously re-implemented the per-row `useState` flags, the confirmation
 * modal and the error handling in every row component.
 *
 * This module owns that policy, framework-free:
 *
 * - the action vocabulary and the per-target routing (`actionFor`),
 * - the confirmation policy as data (`needsConfirmation`, also on the route),
 * - one per-item action state (`idle | pending(verb)`) with coalescing: a burst
 *   of clicks on one row runs a single request, distinct rows run in parallel,
 * - one failure policy: the item resets to idle, the error is surfaced through
 *   `onError` (the adapter wires it to the existing toast) and **no** refresh
 *   happens; a success resets the item and calls `onRefresh`.
 *
 * The module never imports React: components read the store through
 * `useSyncExternalStore(store.subscribe, () => store.getState(id))`. The idle
 * snapshot is a shared frozen constant, so the value is stable across renders
 * (and safe to use as the server snapshot).
 */

import type { DdlProviderDownloadDto, StartDdlProviderDownloadRequest } from "@/lib/api/ddl-providers";

import { browserApi } from "@/lib/api/browser";
import { ApiError } from "@/lib/api/errors";

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

/** The download targets the backoffice can act on. */
export type DownloadTarget =
  | "torrent"
  | "alldebrid"
  | "ddl"
  | "telegram"
  /** One release of an "available downloads" entry (Prowlarr or DDL). */
  | "release"
  /** Every detection entry backing one available group ("dismiss these results"). */
  | "entries";

/**
 * The verbs shared by those targets. `start` creates a download from a
 * discovery row (an available release, an available Telegram book); `retry`
 * replays an existing download. `dismiss` is the soft, non-destructive removal
 * (the row is hidden and, for Telegram/DDL, blacklisted); `delete` is the
 * destructive one.
 */
export type DownloadVerb = "retry" | "start" | "delete" | "dismiss";

export const DOWNLOAD_TARGETS: readonly DownloadTarget[] = [
  "torrent",
  "alldebrid",
  "ddl",
  "telegram",
  "release",
  "entries",
];

export const DOWNLOAD_VERBS: readonly DownloadVerb[] = ["retry", "start", "delete", "dismiss"];

export interface DownloadAction {
  readonly target: DownloadTarget;
  readonly verb: DownloadVerb;
}

/** A new DDL download created from a release of the available section. */
export interface DdlStartContext {
  readonly provider: string;
  readonly body: StartDdlProviderDownloadRequest;
}

/** The available release a `release.delete` / `release.dismiss` removes. */
export interface AvailableReleaseContext {
  readonly entryId: string;
  readonly releaseIndex: number;
  /**
   * Title of the release the user acted on. Sent with the index so the API
   * refuses (`409`) to remove another release when the list shifted.
   */
  readonly title: string;
}

/**
 * Per-call data a route cannot derive from the item id alone. The item id is
 * only the busy key: every route that needs addressing data takes it here.
 */
export interface DownloadActionContext {
  /** The DDL row a retry replays; it must be the row requested by `id`. */
  readonly ddlDownload?: DdlProviderDownloadDto;
  /** The DDL release a `ddl.start` snatches. */
  readonly ddlStart?: DdlStartContext;
  /** The available release a `release.*` action removes. */
  readonly release?: AvailableReleaseContext;
  /** The detection entries an `entries.delete` removes (group "dismiss all"). */
  readonly entryIds?: readonly string[];
}

// ---------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------

export interface DownloadActionRoute {
  readonly target: DownloadTarget;
  readonly verb: DownloadVerb;
  /** Whether the adapter must ask the user before calling `run`. */
  readonly confirm: boolean;
  run(id: string, context?: DownloadActionContext): Promise<void>;
}

type RouteSpec = Pick<DownloadActionRoute, "confirm" | "run">;

/** Unsupported verbs are absent from the table; `actionFor` returns `null`. */
type RouteTable = {
  readonly [target in DownloadTarget]: { readonly [verb in DownloadVerb]?: RouteSpec };
};

/**
 * Replay a DDL release. The server re-creates the download from the release
 * identity, so the row payload is required and must be the row being retried.
 */
async function retryDdlDownload(id: string, context: DownloadActionContext = {}): Promise<void> {
  const download = context.ddlDownload;
  if (!download) {
    throw new Error("ddl retry needs the download row (context.ddlDownload)");
  }
  if (download.id !== id) {
    throw new Error(`ddl retry row mismatch: expected ${id}, got ${download.id}`);
  }
  if (!download.library_id) {
    throw new Error("ddl retry needs a library on the download row");
  }
  await browserApi.ddl.startDownload(download.provider, {
    external_id: download.external_id,
    filename: download.filename,
    size: download.file_size,
    library_id: download.library_id,
    series_name: download.series_name ?? download.filename,
    volume_number: download.volume_number,
  });
}

export class UnsupportedDownloadActionError extends Error {
  readonly action: DownloadAction;

  constructor(action: DownloadAction) {
    super(`download action "${action.verb}" is not supported for target "${action.target}"`);
    this.name = "UnsupportedDownloadActionError";
    this.action = action;
  }
}

/** Replay a release discovered by the DDL search (a brand-new download). */
async function startDdlDownload(
  _id: string,
  context: DownloadActionContext = {},
): Promise<void> {
  const start = context.ddlStart;
  if (!start) {
    throw new Error("ddl start needs the release (context.ddlStart)");
  }
  await browserApi.ddl.startDownload(start.provider, start.body);
}

/**
 * Remove one release from its available entry. `blacklist` also hides the
 * release from future detection runs, which is the section's "don't show
 * again" verb (`dismiss`).
 */
async function removeAvailableRelease(
  _id: string,
  context: DownloadActionContext = {},
  blacklist: boolean,
): Promise<void> {
  const release = context.release;
  if (!release) {
    throw new Error("available release removal needs the row (context.release)");
  }
  await browserApi.downloads.available.removeRelease(
    release.entryId,
    release.releaseIndex,
    blacklist,
    release.title,
  );
}

/**
 * The API refused an index-based release removal because the release at that
 * index no longer has the expected title (`409`): nothing changed, but the
 * list on screen is stale and must be refreshed.
 */
export function isStaleReleaseError(error: unknown, action: DownloadAction): boolean {
  return action.target === "release" && error instanceof ApiError && error.status === 409;
}

/** Remove every detection entry of one available group ("dismiss these results"). */
async function removeAvailableEntries(
  _id: string,
  context: DownloadActionContext = {},
): Promise<void> {
  const entryIds = context.entryIds;
  if (!entryIds || entryIds.length === 0) {
    throw new Error("available group removal needs its entries (context.entryIds)");
  }
  await Promise.all(entryIds.map((entryId) => browserApi.downloads.available.removeEntry(entryId)));
}

/**
 * Endpoint matrix. Confirmations mirror the current UI: every destructive or
 * hiding removal asks first, retries and starts never do, the Telegram
 * `dismiss` (the reversible "hide this release" action) stays immediate, and
 * the available-section removals stay immediate (they only drop a detection
 * row, which the next search recreates).
 *
 * A row may still pass its own prompt for a verb whose mandatory confirmation
 * is `false`: the adapter always asks when a prompt is supplied.
 */
const ROUTES: RouteTable = {
  torrent: {
    retry: { confirm: false, run: (id) => browserApi.downloads.torrents.retry(id) },
    delete: { confirm: true, run: (id) => browserApi.downloads.torrents.remove(id) },
  },
  alldebrid: {
    // A failed magnet never produced local files: restart the magnet itself.
    retry: { confirm: false, run: (id) => browserApi.downloads.alldebrid.restart(id) },
    // The row is a torrent download whichever client produced it.
    delete: { confirm: true, run: (id) => browserApi.downloads.torrents.remove(id) },
  },
  ddl: {
    retry: { confirm: false, run: retryDdlDownload },
    start: { confirm: false, run: startDdlDownload },
    dismiss: { confirm: true, run: (id) => browserApi.ddl.dismissDownload(id, false) },
    delete: { confirm: true, run: (id) => browserApi.ddl.dismissDownload(id, true) },
  },
  telegram: {
    retry: { confirm: false, run: (id) => browserApi.telegram.downloadBook(id) },
    start: { confirm: false, run: (id) => browserApi.telegram.downloadBook(id) },
    // Soft delete: the row is dismissed and blacklisted.
    dismiss: { confirm: false, run: (id) => browserApi.telegram.deleteBook(id, false) },
    delete: { confirm: true, run: (id) => browserApi.telegram.deleteBook(id, true) },
  },
  release: {
    dismiss: { confirm: false, run: (id, ctx) => removeAvailableRelease(id, ctx, true) },
    delete: { confirm: false, run: (id, ctx) => removeAvailableRelease(id, ctx, false) },
  },
  entries: {
    delete: { confirm: false, run: removeAvailableEntries },
  },
};

/** The route for one `(target, verb)` pair, or `null` when unsupported. */
export function actionFor(
  target: DownloadTarget,
  verb: DownloadVerb,
): DownloadActionRoute | null {
  const spec = ROUTES[target]?.[verb];
  return spec ? { target, verb, confirm: spec.confirm, run: spec.run } : null;
}

/** Whether `(target, verb)` is wired to an endpoint. */
export function supportsDownloadAction(target: DownloadTarget, verb: DownloadVerb): boolean {
  return actionFor(target, verb) !== null;
}

/** Whether the adapter must ask the user before running that action. */
export function needsConfirmation(target: DownloadTarget, verb: DownloadVerb): boolean {
  return actionFor(target, verb)?.confirm ?? false;
}

// ---------------------------------------------------------------------------
// Per-item action state
// ---------------------------------------------------------------------------

export type DownloadItemActionState =
  | { readonly status: "idle" }
  | { readonly status: "pending"; readonly verb: DownloadVerb };

/**
 * Shared idle snapshot. Untouched items all return this exact object, which is
 * what keeps `useSyncExternalStore` snapshots stable.
 */
export const IDLE_DOWNLOAD_ACTION_STATE: DownloadItemActionState = Object.freeze({
  status: "idle",
});

export interface DownloadActionStoreOptions {
  /**
   * Called after a successful action, with the action and the item id. Wire it
   * to the row refresh; it is never called on failure.
   */
  onRefresh?: (action: DownloadAction, id: string) => void;
  /**
   * Called on every failure, after the item has been reset to idle. Wire it to
   * the existing toast: the store deliberately does not swallow errors.
   */
  onError?: (error: unknown, action: DownloadAction, id: string) => void;
}

export interface DownloadActionStore {
  /** `useSyncExternalStore` subscriber. */
  subscribe(listener: () => void): () => void;
  /** Stable snapshot for one item; also usable as the server snapshot. */
  getState(id: string): DownloadItemActionState;
  /** Whether one item currently has an in-flight action. */
  busy(id: string): boolean;
  /**
   * Run one action on one item. Concurrent calls for the same id are coalesced
   * onto the in-flight request; distinct ids run concurrently.
   *
   * The returned promise settles when the item is back to idle. Failures do not
   * reject: they reset the item and reach `onError`, so a fire-and-forget click
   * handler cannot trigger an unhandled rejection. Confirmations are the
   * adapter's job (`needsConfirmation`), the store never prompts.
   */
  run(action: DownloadAction, id: string, context?: DownloadActionContext): Promise<void>;
  /** Clear the visual state of one item, or of every item. */
  reset(id?: string): void;
  /** Drop listeners and state; in-flight requests still settle silently. */
  dispose(): void;
}

export function createDownloadActionStore(
  options: DownloadActionStoreOptions = {},
): DownloadActionStore {
  const listeners = new Set<() => void>();
  const states = new Map<string, DownloadItemActionState>();
  const inFlight = new Map<string, Promise<void>>();
  let disposed = false;

  function notify() {
    if (disposed) return;
    for (const listener of Array.from(listeners)) listener();
  }

  function setState(id: string, state: DownloadItemActionState) {
    if (disposed) return;
    if (state.status === "idle") states.delete(id);
    else states.set(id, state);
    notify();
  }

  function getState(id: string): DownloadItemActionState {
    return states.get(id) ?? IDLE_DOWNLOAD_ACTION_STATE;
  }

  function settle(id: string, resolve: () => void) {
    inFlight.delete(id);
    resolve();
  }

  function run(
    action: DownloadAction,
    id: string,
    context: DownloadActionContext = {},
  ): Promise<void> {
    const pending = inFlight.get(id);
    if (pending) return pending;

    const route = actionFor(action.target, action.verb);

    let resolveSettled!: () => void;
    const settled = new Promise<void>((resolve) => {
      resolveSettled = resolve;
    });
    // Register before notifying: a listener reacting to the pending state must
    // not be able to start a second request on the same item.
    inFlight.set(id, settled);
    setState(id, { status: "pending", verb: action.verb });

    void (async () => {
      try {
        if (!route) throw new UnsupportedDownloadActionError(action);
        await route.run(id, context);
      } catch (error) {
        setState(id, IDLE_DOWNLOAD_ACTION_STATE);
        options.onError?.(error, action, id);
        return;
      } finally {
        settle(id, resolveSettled);
      }
      setState(id, IDLE_DOWNLOAD_ACTION_STATE);
      options.onRefresh?.(action, id);
    })();

    return settled;
  }

  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    getState,
    busy(id) {
      return getState(id).status === "pending";
    },
    run,
    reset(id) {
      if (id === undefined) {
        states.clear();
        notify();
        return;
      }
      if (!states.has(id)) return;
      states.delete(id);
      notify();
    },
    dispose() {
      disposed = true;
      listeners.clear();
      states.clear();
      inFlight.clear();
    },
  };
}
