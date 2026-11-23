/**
 * Write seam for the two acquisition search modals (Telegram live search and
 * DDL provider search).
 *
 * Both modals act on a result row: start (or retry) its download, or dismiss
 * it. Each one replayed the same protocol by hand — one `Set` of in-flight ids
 * per verb, a hand-written optimistic status written back into the result rows,
 * a `prevStatus` captured before the call to restore on failure, and a
 * `catch {}` that silently swallowed a rejected dismiss. The rows are a *read*
 * model owned by the modal; writing the optimistic badge into them made the
 * rollback depend on whichever row object the closure had captured.
 *
 * This module owns that protocol, framework-free:
 *
 * - one vocabulary (`SearchResultAction`: `source` × `verb`) routed through the
 *   existing acquisition routes of `lib/downloads/actions.ts` — there is one
 *   route table, not a second one per modal;
 * - one identity per result row: `searchKey(provider, externalId)`, so the two
 *   modals share their overlay bookkeeping;
 * - one optimistic overlay (`status(key)`): the badge is written synchronously
 *   when `run` is called, before the request even starts, and it survives the
 *   success (the server read may lag behind);
 * - per-key serialisation: a burst on one row runs one request at a time,
 *   distinct rows run in parallel;
 * - one rollback policy: a failure restores the last overlay the server
 *   acknowledged (or removes the overlay when there is none), guarded by a
 *   per-key token so a failing earlier write cannot wipe a newer overlay, then
 *   reaches `onError` — the store never swallows an error.
 *
 * The store never invents a result row and never refetches: `clear(keys)` drops
 * the overlays a fresh server read supersedes (the DDL poll), `reset()` drops
 * everything when a new search replaces the rows.
 */

import {
  actionFor,
  UnsupportedDownloadActionError,
  type DdlStartContext,
  type DownloadActionContext,
  type DownloadTarget,
} from "@/lib/downloads/actions";

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

/** The two search surfaces that can act on a result row. */
export type SearchSource = Extract<DownloadTarget, "telegram" | "ddl">;

/**
 * The verbs a search row exposes. `start` creates a download (and replays a
 * failed or imported one); `dismiss` hides the row without deleting files.
 */
export type SearchVerb = "start" | "dismiss";

export interface SearchResultAction {
  readonly source: SearchSource;
  readonly verb: SearchVerb;
}

/** The optimistic badge a running action puts on its row. */
export type SearchResultStatus = "queued" | "dismissed";

/** The optimistic status a verb shows until the server read catches up. */
export function optimisticStatus(verb: SearchVerb): SearchResultStatus {
  return verb === "start" ? "queued" : "dismissed";
}

/**
 * The identity of one search result, shared by both modals. A result found on
 * two providers (same `external_id`) stays two distinct rows.
 */
export function searchKey(provider: string, externalId: string): string {
  return `${provider}:${externalId}`;
}

/**
 * The `external_id` half of a key produced by `searchKey`. The provider half is
 * never an address: the acquisition routes take the external id (or, for a DDL
 * dismiss, the download row id supplied by the caller).
 */
export function externalIdOf(key: string): string {
  const separator = key.indexOf(":");
  return separator === -1 ? key : key.slice(separator + 1);
}

/**
 * Per-call data the acquisition routes cannot derive from the key alone.
 *
 * A key names a *release* (`provider:external_id`); a DDL dismiss addresses the
 * *download row* it produced, and a DDL start carries the release payload the
 * endpoint snatches.
 */
export interface SearchActionContext {
  /** DDL start: the provider and the request body the release snatches. */
  readonly ddlStart?: DdlStartContext;
  /** DDL dismiss: the download row to remove (the key carries the release). */
  readonly downloadId?: string;
}

// ---------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------

/** The slice of an acquisition route the store calls. */
export interface SearchActionRoute {
  run(id: string, context?: DownloadActionContext): Promise<void>;
}

/** Resolves an action to its route, or `null` when the pair is unsupported. */
export type SearchRouteLookup = (action: SearchResultAction) => SearchActionRoute | null;

/**
 * The production table: the acquisition routes. `telegram.start` and
 * `telegram.retry` already share `downloadBook`, `telegram.dismiss` is the soft
 * delete, `ddl.start` snatches the release and `ddl.dismiss` is the soft
 * removal. A second table here would drift from the downloads page.
 */
const acquisitionRoute: SearchRouteLookup = (action) => actionFor(action.source, action.verb);

/** The id and context one route call takes. */
interface SearchWrite {
  readonly id: string;
  readonly context?: DownloadActionContext;
}

/**
 * Translate a keyed action into the address its endpoint takes. The key is the
 * overlay identity; the route id is the release external id, except for a DDL
 * dismiss which removes the download row.
 */
function writeFor(
  action: SearchResultAction,
  key: string,
  context: SearchActionContext = {},
): SearchWrite {
  const externalId = externalIdOf(key);
  if (action.source === "telegram") {
    return { id: externalId };
  }
  if (action.verb === "dismiss") {
    if (!context.downloadId) {
      throw new Error("ddl dismiss needs the download row id (context.downloadId)");
    }
    return { id: context.downloadId };
  }
  if (!context.ddlStart) {
    throw new Error("ddl start needs the release (context.ddlStart)");
  }
  return { id: externalId, context: { ddlStart: context.ddlStart } };
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export interface SearchActionStoreOptions {
  /** Route table; defaults to the acquisition routes of the downloads page. */
  route?: SearchRouteLookup;
  /**
   * Called on every failure, after the overlay has been rolled back. Wire it to
   * the existing toast: the store deliberately does not swallow errors (a
   * rejected dismiss used to be silent).
   */
  onError?: (error: unknown, action: SearchResultAction, key: string) => void;
}

export interface SearchActionStore {
  /** `useSyncExternalStore` subscriber. */
  subscribe(listener: () => void): () => void;
  /** Bumped on every overlay/pending change; a stable snapshot. */
  version(): number;
  /** Whether the row currently has a queued or in-flight write. */
  pending(key: string): boolean;
  /** The optimistic badge of the row, or `null` when the server status shows. */
  status(key: string): SearchResultStatus | null;
  /**
   * Run one action on one row. The optimistic badge is written synchronously;
   * writes on the same key are serialised behind each other, distinct keys run
   * in parallel. The promise never rejects: it resolves `true` when the route
   * accepted the write and `false` after `onError`.
   */
  run(action: SearchResultAction, key: string, context?: SearchActionContext): Promise<boolean>;
  /**
   * Drop the overlays (and their acknowledged value) for keys a fresh server
   * read supersedes. A key with an in-flight write keeps its overlay: the
   * request is still the freshest information for that row.
   */
  clear(keys: readonly string[]): void;
  /**
   * Drop every overlay when a new search replaces the rows. In-flight writes
   * keep running and keep `pending` true, but no longer write state back.
   */
  reset(): void;
  /** Drop listeners and state; in-flight requests still settle silently. */
  dispose(): void;
}

interface Overlay {
  readonly status: SearchResultStatus;
  /** Identity of the write that owns the overlay. */
  readonly token: number;
}

export function createSearchActionStore(
  options: SearchActionStoreOptions = {},
): SearchActionStore {
  const route = options.route ?? acquisitionRoute;
  const listeners = new Set<() => void>();
  /** Current optimistic badge per row. */
  const overlays = new Map<string, Overlay>();
  /** Last overlay a server write acknowledged; a failed newer write falls back to it. */
  const acknowledged = new Map<string, SearchResultStatus>();
  /** Tail of the per-key queue. */
  const tails = new Map<string, Promise<void>>();
  /** Queued + running writes per key. */
  const pendingCounts = new Map<string, number>();
  let revision = 0;
  let tokenSeq = 0;
  /** Bumped by `reset()`: a write enqueued before it must not write state back. */
  let generation = 0;
  let disposed = false;

  function touch() {
    if (disposed) return;
    revision += 1;
    for (const listener of Array.from(listeners)) listener();
  }

  function rollback(key: string, token: number) {
    if (overlays.get(key)?.token !== token) return;
    const acknowledgedStatus = acknowledged.get(key);
    if (acknowledgedStatus) overlays.set(key, { status: acknowledgedStatus, token });
    else overlays.delete(key);
  }

  async function perform(
    action: SearchResultAction,
    key: string,
    context: SearchActionContext,
    token: number,
    startedIn: number,
  ): Promise<boolean> {
    try {
      const target = route(action);
      if (!target) {
        throw new UnsupportedDownloadActionError({
          target: action.source,
          verb: action.verb,
        });
      }
      const write = writeFor(action, key, context);
      await target.run(write.id, write.context);
    } catch (error) {
      if (startedIn === generation) {
        rollback(key, token);
        touch();
      }
      try {
        options.onError?.(error, action, key);
      } catch {
        // A broken toast must not break the row's queue.
      }
      return false;
    }

    if (startedIn === generation) {
      // The server accepted the write: remember the badge a later failure of a
      // newer write on this row must fall back to.
      acknowledged.set(key, optimisticStatus(action.verb));
    }
    return true;
  }

  function run(
    action: SearchResultAction,
    key: string,
    context: SearchActionContext = {},
  ): Promise<boolean> {
    const token = ++tokenSeq;
    const startedIn = generation;
    const previous = tails.get(key) ?? Promise.resolve();
    // Register the queue and the optimistic overlay before notifying: a
    // listener reacting to the badge must not be able to start a parallel write
    // on the same row.
    const result = previous.then(
      () => perform(action, key, context, token, startedIn),
      () => perform(action, key, context, token, startedIn),
    );
    const tail = result.then(
      () => undefined,
      () => undefined,
    );
    tails.set(key, tail);
    pendingCounts.set(key, (pendingCounts.get(key) ?? 0) + 1);
    overlays.set(key, { status: optimisticStatus(action.verb), token });
    touch();

    void result.then(() => {
      if (tails.get(key) === tail) tails.delete(key);
      const remaining = (pendingCounts.get(key) ?? 0) - 1;
      if (remaining > 0) pendingCounts.set(key, remaining);
      else pendingCounts.delete(key);
      touch();
    });

    return result;
  }

  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    version() {
      return revision;
    },
    pending(key) {
      return (pendingCounts.get(key) ?? 0) > 0;
    },
    status(key) {
      return overlays.get(key)?.status ?? null;
    },
    run,
    clear(keys) {
      let changed = false;
      for (const key of keys) {
        if ((pendingCounts.get(key) ?? 0) > 0) continue;
        if (overlays.delete(key)) changed = true;
        if (acknowledged.delete(key)) changed = true;
      }
      if (changed) touch();
    },
    reset() {
      generation += 1;
      const changed = overlays.size > 0 || acknowledged.size > 0;
      overlays.clear();
      acknowledged.clear();
      if (changed) touch();
    },
    dispose() {
      disposed = true;
      listeners.clear();
      overlays.clear();
      acknowledged.clear();
      tails.clear();
      pendingCounts.clear();
    },
  };
}
