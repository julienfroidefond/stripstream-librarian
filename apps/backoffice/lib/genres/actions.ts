/**
 * Write seam for the genre workshop (rename / delete / assign / AI tag).
 *
 * The genres page writes four times: it renames a genre, deletes one, assigns
 * a genre to the selected series and applies an AI-suggested tag. Each workflow
 * used to replay the same `setBusy(true)` / try / reload / toast / finally
 * skeleton and share one global `busy` flag, so renaming one genre blocked the
 * assignment of another and the concurrency rules were only covered by an
 * end-to-end spec.
 *
 * This module owns that protocol, framework-free:
 *
 * - one write vocabulary (`GenreWrite`) routed to `browserApi.genres`;
 * - the **subject** of a write is the genre name (the API writes per genre):
 *   distinct genres run in parallel, writes on one genre are serialised, and an
 *   identical write (same verb, same arguments, sorted series ids) is coalesced
 *   onto the in-flight request;
 * - `busy(genre)` is true from the call until every write on that genre has
 *   settled, **including** the caller's optional aftermath;
 * - one failure policy: the write resolves `false`, `onError` surfaces the
 *   error and the next write on the same subject still runs. The store never
 *   reloads the page — reconciling the browse read model (ADR 0028) is the
 *   caller's job, passed as the write's aftermath.
 */

export type GenreWrite =
  | { readonly type: "rename"; readonly from: string; readonly to: string }
  | { readonly type: "delete"; readonly name: string }
  | { readonly type: "assign"; readonly genre: string; readonly seriesIds: readonly string[] }
  | { readonly type: "applySuggestion"; readonly genre: string; readonly seriesId: string };

/** The slice of `browserApi.genres` the store writes through. */
export interface GenreActionsApi {
  rename(oldName: string, newName: string): Promise<void>;
  remove(name: string): Promise<void>;
  assign(genre: string, seriesIds: string[]): Promise<void>;
}

export interface GenreActionStoreOptions {
  api: GenreActionsApi;
  /**
   * Called on every failure, after the write has settled. Wire it to the
   * existing toast: the store deliberately does not swallow errors.
   */
  onError?: (error: unknown, write: GenreWrite) => void;
}

export interface GenreActionStore {
  /** `useSyncExternalStore` subscriber. */
  subscribe(listener: () => void): () => void;
  /** Bumped on every busy change; a stable snapshot for `useSyncExternalStore`. */
  version(): number;
  /** Whether the genre currently has a queued or in-flight write. */
  busy(genre: string): boolean;
  /**
   * Run one write. An identical write in flight is returned as is; writes on
   * the same genre are queued behind each other; distinct genres run in
   * parallel.
   *
   * `after` runs only when the API accepted the write, before the subject is
   * released: it is the caller's read reconciliation (selection transition,
   * reload, success toast). Identical coalesced calls share the first call's
   * aftermath. It must not reject; a throw is swallowed so a successful write
   * never becomes a failure.
   *
   * The promise never rejects: a write failure resolves `false` after
   * `onError`. Confirmations belong to the caller.
   */
  run(write: GenreWrite, after?: () => Promise<void> | void): Promise<boolean>;
}

/** Stable identity of one write, used to coalesce a burst of identical calls. */
function writeKey(write: GenreWrite): string {
  switch (write.type) {
    case "rename":
      return `rename\u0000${write.from}\u0000${write.to}`;
    case "delete":
      return `delete\u0000${write.name}`;
    case "assign":
      return `assign\u0000${write.genre}\u0000${[...write.seriesIds].sort().join("\u0001")}`;
    case "applySuggestion":
      return `applySuggestion\u0000${write.genre}\u0000${write.seriesId}`;
  }
}

/** The genre a write acts on; writes on one subject are serialised. */
function subjectOf(write: GenreWrite): string {
  switch (write.type) {
    case "rename":
      return write.from;
    case "delete":
      return write.name;
    case "assign":
    case "applySuggestion":
      return write.genre;
  }
}

export function createGenreActionStore({
  api,
  onError,
}: GenreActionStoreOptions): GenreActionStore {
  const listeners = new Set<() => void>();
  /** Identical writes currently queued or running, by `writeKey`. */
  const inFlight = new Map<string, Promise<boolean>>();
  /** Tail of the queue per subject (genre name). */
  const locks = new Map<string, Promise<void>>();
  /** Number of queued or running writes per subject. */
  const pending = new Map<string, number>();
  let revision = 0;

  function notify() {
    for (const listener of Array.from(listeners)) listener();
  }

  function bump(genre: string, delta: number) {
    const next = (pending.get(genre) ?? 0) + delta;
    if (next > 0) pending.set(genre, next);
    else pending.delete(genre);
    revision += 1;
    notify();
  }

  async function perform(
    write: GenreWrite,
    after?: () => Promise<void> | void,
  ): Promise<boolean> {
    try {
      switch (write.type) {
        case "rename":
          await api.rename(write.from, write.to);
          break;
        case "delete":
          await api.remove(write.name);
          break;
        case "assign":
          await api.assign(write.genre, [...write.seriesIds]);
          break;
        case "applySuggestion":
          await api.assign(write.genre, [write.seriesId]);
          break;
      }
    } catch (error) {
      try {
        onError?.(error, write);
      } catch {
        // A broken toast must not break the subject's queue.
      }
      return false;
    }

    try {
      await after?.();
    } catch {
      // The API already accepted the write; the caller's read policy owns its
      // own errors (the page reload toasts them). Keep the queue alive.
    }
    return true;
  }

  function run(write: GenreWrite, after?: () => Promise<void> | void): Promise<boolean> {
    const key = writeKey(write);
    const running = inFlight.get(key);
    if (running) return running;

    const subject = subjectOf(write);
    const previous = locks.get(subject) ?? Promise.resolve();
    // Run after the previous write on this subject, whether it succeeded or
    // failed: one rejection must not cancel the queue behind it.
    const result = previous.then(
      () => perform(write, after),
      () => perform(write, after),
    );
    const tail = result.then(() => undefined, () => undefined);
    locks.set(subject, tail);
    // Register before notifying, so a listener reacting to the busy state
    // cannot start a second request for the same write.
    inFlight.set(key, result);
    bump(subject, 1);

    void result.then(() => {
      if (inFlight.get(key) === result) inFlight.delete(key);
      if (locks.get(subject) === tail) locks.delete(subject);
      bump(subject, -1);
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
    busy(genre) {
      return (pending.get(genre) ?? 0) > 0;
    },
    run,
  };
}
