/**
 * Write seam for reading lists (create / rename / delete / add / remove /
 * move / reorder / moveToList).
 *
 * Four surfaces write reading lists: the overview page, the detail page, the
 * "add to list" modal of a series and the AI suggestions modal. They used to
 * hold their own copy of the data in `useState(initial)`, fabricate items the
 * API never returned (`POST .../series` answers `204`) and apply four failure
 * policies. This module owns that protocol, framework-free:
 *
 * - one operation vocabulary (`ReadingListOp`) routed to `browserApi`;
 * - writes are **server-confirmed**: nothing is shown before the API accepts
 *   it, then the touched data is reconciled from the server (`detail()` for a
 *   tracked list, `list()` for a tracked overview) — never from a local guess;
 * - the only optimistic write is the reorder (`move`, `reorder`): the new
 *   order shows at once and a failure restores the last order the server
 *   acknowledged, read from the store (never from a stale closure);
 * - writes touching one list are serialised, so a burst of reorders reaches
 *   the server in click order; a burst of clicks on the same item (same
 *   `opKey`) is coalesced onto the in-flight request;
 * - one failure policy: the state is left (or put back) as the server knows
 *   it, `onError` is called (the adapter wires it to the toast) and `onSuccess`
 *   (the adapter's `router.refresh()`) is only called when something applied.
 *
 * The module never imports React: the adapter reads `getState()` through
 * `useSyncExternalStore`. The snapshot object only changes on a notification.
 */

import type {
  ReadingListDetailDto,
  ReadingListDto,
  ReadingListSeriesDto,
} from "@/lib/api/reading-lists";

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

export type ReadingListOp =
  | {
      readonly type: "create";
      readonly name: string;
      readonly description?: string | null;
      /**
       * Series to put in the new list. When every one of them fails, the list
       * is removed again so no empty list is left behind.
       */
      readonly seriesIds?: readonly string[];
    }
  | {
      readonly type: "rename";
      readonly listId: string;
      readonly name: string;
      readonly description: string | null;
    }
  | { readonly type: "delete"; readonly listId: string }
  | { readonly type: "add"; readonly listId: string; readonly seriesId: string }
  | { readonly type: "remove"; readonly listId: string; readonly seriesId: string }
  /** Move one series one step inside its list (optimistic reorder). */
  | {
      readonly type: "move";
      readonly listId: string;
      readonly seriesId: string;
      readonly direction: "up" | "down";
    }
  /** Replace the whole order of a list (optimistic). */
  | { readonly type: "reorder"; readonly listId: string; readonly seriesIds: readonly string[] }
  | {
      readonly type: "moveToList";
      readonly fromListId: string;
      readonly toListId: string;
      readonly seriesId: string;
    };

export type ReadingListOpType = ReadingListOp["type"];

/** The `browserApi.readingLists` subset the store drives (injectable in tests). */
export interface ReadingListsApi {
  list(): Promise<ReadingListDto[]>;
  detail(listId: string): Promise<ReadingListDetailDto>;
  create(name: string, description?: string | null): Promise<ReadingListDto>;
  update(
    listId: string,
    data: { name?: string; description?: string | null },
  ): Promise<ReadingListDto>;
  remove(listId: string): Promise<void>;
  addSeries(listId: string, seriesId: string): Promise<void>;
  removeSeries(listId: string, seriesId: string): Promise<void>;
  moveSeries(fromListId: string, seriesId: string, toListId: string): Promise<void>;
  reorder(listId: string, seriesIds: string[]): Promise<void>;
}

/**
 * The busy key of an operation: buttons read `pending(opKey(op))`. Reorders
 * have no key — they are optimistic and never block the controls.
 */
export function opKey(op: ReadingListOp): string | null {
  switch (op.type) {
    case "create":
      return "create";
    case "rename":
    case "delete":
      return `list:${op.listId}`;
    case "add":
    case "remove":
      return `item:${op.listId}:${op.seriesId}`;
    case "moveToList":
      return `item:${op.fromListId}:${op.seriesId}`;
    case "move":
    case "reorder":
      return null;
  }
}

/** The lists whose writes must not interleave with this operation. */
function lockedLists(op: ReadingListOp): string[] {
  switch (op.type) {
    case "create":
      return [];
    case "moveToList":
      return [op.fromListId, op.toListId];
    default:
      return [op.listId];
  }
}

/** Overview order: the page lists reading lists by name. */
export function sortLists(lists: readonly ReadingListDto[]): ReadingListDto[] {
  return [...lists].sort((a, b) => a.name.localeCompare(b.name));
}

/** The order after moving `seriesId` one step, or `null` when it cannot move. */
export function movedOrder(
  items: readonly Pick<ReadingListSeriesDto, "id">[],
  seriesId: string,
  direction: "up" | "down",
): string[] | null {
  const ids = items.map((item) => item.id);
  const index = ids.indexOf(seriesId);
  const target = direction === "up" ? index - 1 : index + 1;
  if (index < 0 || target < 0 || target >= ids.length) return null;
  [ids[index], ids[target]] = [ids[target], ids[index]];
  return ids;
}

/** `items` sorted by `order`; ids missing from `order` keep their relative place at the end. */
function applyOrder(
  items: readonly ReadingListSeriesDto[],
  order: readonly string[],
): ReadingListSeriesDto[] {
  const rank = new Map(order.map((id, index) => [id, index]));
  return items
    .map((item, index) => ({ item, index }))
    .sort((a, b) => {
      const ra = rank.get(a.item.id) ?? order.length + a.index;
      const rb = rank.get(b.item.id) ?? order.length + b.index;
      return ra - rb;
    })
    .map(({ item }, position) => ({ ...item, position }));
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

export interface ReadingListsSnapshot {
  /** The tracked overview, or `null` when this store does not show one. */
  readonly lists: readonly ReadingListDto[] | null;
  /** The tracked list details, by list id. */
  readonly details: Readonly<Record<string, ReadingListDetailDto>>;
  /** The `opKey`s with a request in flight. */
  readonly pending: ReadonlySet<string>;
}

/** What one `run` / `runAll` produced. `ok` means nothing failed. */
export interface ReadingListOutcome {
  readonly ok: boolean;
  /** Writes the server accepted. */
  readonly applied: number;
  /** Writes the server rejected. */
  readonly failed: number;
  /** The last failure, when any. */
  readonly error: unknown;
  /** The list a successful `create` produced (the last one for a batch). */
  readonly created: ReadingListDto | null;
}

export interface ReadingListsStoreOptions<Meta = unknown> {
  api: ReadingListsApi;
  lists?: readonly ReadingListDto[] | null;
  details?: readonly ReadingListDetailDto[];
  /** Called once per `run` / `runAll` when at least one write applied. */
  onSuccess?: (outcome: ReadingListOutcome, meta: Meta | undefined) => void;
  /**
   * Called once per `run` / `runAll` when at least one write failed (after the
   * state was put back), and when the reconciliation read fails.
   */
  onError?: (error: unknown, outcome: ReadingListOutcome, meta: Meta | undefined) => void;
}

export interface ReadingListsStore<Meta = unknown> {
  subscribe(listener: () => void): () => void;
  getState(): ReadingListsSnapshot;
  pending(key: string): boolean;
  /** Start tracking an overview / a detail read by the caller. */
  hydrate(data: { lists?: readonly ReadingListDto[]; detail?: ReadingListDetailDto }): void;
  /** Run one operation. Never rejects: failures reach `onError`. */
  run(op: ReadingListOp, meta?: Meta): Promise<ReadingListOutcome>;
  /**
   * Run several operations in order, as one user action: one reconciliation,
   * at most one `onSuccess` and one `onError` for the whole batch.
   */
  runAll(ops: readonly ReadingListOp[], meta?: Meta): Promise<ReadingListOutcome>;
  dispose(): void;
}

interface Tally {
  applied: number;
  failed: number;
  error: unknown;
  created: ReadingListDto | null;
  /** Lists whose detail must be re-read from the server. */
  touched: Set<string>;
  /** Whether the overview must be re-read. */
  overview: boolean;
}

const EMPTY_PENDING: ReadonlySet<string> = new Set();

export function createReadingListsStore<Meta = unknown>(
  options: ReadingListsStoreOptions<Meta>,
): ReadingListsStore<Meta> {
  const { api } = options;
  const listeners = new Set<() => void>();
  const inFlight = new Map<string, Promise<ReadingListOutcome>>();
  const locks = new Map<string, Promise<unknown>>();
  /** Last order the server acknowledged, per tracked list. */
  const confirmedOrder = new Map<string, string[]>();
  /** Bumped by every optimistic reorder: only the latest one may roll back. */
  const reorderSeq = new Map<string, number>();
  let disposed = false;

  const initialDetails: Record<string, ReadingListDetailDto> = {};
  for (const detail of options.details ?? []) {
    initialDetails[detail.id] = detail;
    confirmedOrder.set(detail.id, detail.items.map((item) => item.id));
  }
  let state: ReadingListsSnapshot = {
    lists: options.lists ? sortLists(options.lists) : null,
    details: initialDetails,
    pending: EMPTY_PENDING,
  };

  function update(patch: Partial<ReadingListsSnapshot>) {
    if (disposed) return;
    state = { ...state, ...patch };
    for (const listener of Array.from(listeners)) listener();
  }

  function setDetail(detail: ReadingListDetailDto, acknowledged: boolean) {
    if (acknowledged) confirmedOrder.set(detail.id, detail.items.map((item) => item.id));
    update({ details: { ...state.details, [detail.id]: detail } });
  }

  function dropList(listId: string) {
    const { [listId]: _dropped, ...details } = state.details;
    void _dropped;
    confirmedOrder.delete(listId);
    update({
      details,
      lists: state.lists ? state.lists.filter((list) => list.id !== listId) : null,
    });
  }

  function setPending(key: string | null, on: boolean) {
    if (!key) return;
    const next = new Set(state.pending);
    if (on) next.add(key);
    else next.delete(key);
    update({ pending: next.size === 0 ? EMPTY_PENDING : next });
  }

  /** Run `task` after every earlier write on the same lists. */
  function withLocks<T>(listIds: readonly string[], task: () => Promise<T>): Promise<T> {
    const previous = Promise.all(listIds.map((id) => locks.get(id)));
    const result = previous.then(task, task);
    const tail = result.catch(() => undefined);
    for (const id of listIds) locks.set(id, tail);
    void tail.then(() => {
      for (const id of listIds) if (locks.get(id) === tail) locks.delete(id);
    });
    return result;
  }

  function fail(tally: Tally, error: unknown) {
    tally.failed += 1;
    tally.error = error;
  }

  /** Optimistic reorder; rolls back to the acknowledged order on failure. */
  async function reorder(listId: string, order: string[], tally: Tally) {
    const seq = (reorderSeq.get(listId) ?? 0) + 1;
    reorderSeq.set(listId, seq);
    const before = state.details[listId];
    if (before) setDetail({ ...before, items: applyOrder(before.items, order) }, false);

    await withLocks([listId], async () => {
      try {
        await api.reorder(listId, order);
        tally.applied += 1;
        confirmedOrder.set(listId, order);
      } catch (error) {
        fail(tally, error);
        // A newer reorder is queued behind this one: it carries the full
        // order and decides the final state, success or rollback.
        if (reorderSeq.get(listId) !== seq) return;
        const current = state.details[listId];
        const acknowledged = confirmedOrder.get(listId);
        if (current && acknowledged) {
          setDetail({ ...current, items: applyOrder(current.items, acknowledged) }, false);
        }
      }
    });
  }

  async function execute(op: ReadingListOp, tally: Tally): Promise<void> {
    if (op.type === "move" || op.type === "reorder") {
      const detail = state.details[op.listId];
      const order =
        op.type === "reorder"
          ? [...op.seriesIds]
          : detail
            ? movedOrder(detail.items, op.seriesId, op.direction)
            : null;
      if (order) await reorder(op.listId, order, tally);
      return;
    }

    await withLocks(lockedLists(op), async () => {
      try {
        switch (op.type) {
          case "create": {
            const created = await api.create(op.name, op.description);
            const seriesIds = op.seriesIds ?? [];
            let added = 0;
            let lastError: unknown = null;
            for (const seriesId of seriesIds) {
              try {
                await api.addSeries(created.id, seriesId);
                added += 1;
              } catch (error) {
                lastError = error;
                fail(tally, error);
              }
            }
            if (seriesIds.length > 0 && added === 0) {
              // Never leave an empty list behind: the creation itself failed.
              await api.remove(created.id).catch(() => undefined);
              tally.error = lastError;
              return;
            }
            tally.applied += Math.max(added, 1);
            tally.created = created;
            if (state.lists) update({ lists: sortLists([...state.lists, created]) });
            tally.overview = seriesIds.length > 0;
            return;
          }
          case "rename": {
            const updated = await api.update(op.listId, {
              name: op.name,
              description: op.description,
            });
            tally.applied += 1;
            if (state.lists) {
              update({
                lists: sortLists(
                  state.lists.map((list) => (list.id === updated.id ? { ...list, ...updated } : list)),
                ),
              });
            }
            const detail = state.details[op.listId];
            if (detail) {
              setDetail(
                {
                  ...detail,
                  name: updated.name,
                  description: updated.description,
                  updated_at: updated.updated_at,
                },
                false,
              );
            }
            return;
          }
          case "delete":
            await api.remove(op.listId);
            tally.applied += 1;
            dropList(op.listId);
            return;
          case "add":
            await api.addSeries(op.listId, op.seriesId);
            tally.applied += 1;
            tally.touched.add(op.listId);
            tally.overview = true;
            return;
          case "remove":
            await api.removeSeries(op.listId, op.seriesId);
            tally.applied += 1;
            tally.touched.add(op.listId);
            tally.overview = true;
            return;
          case "moveToList":
            await api.moveSeries(op.fromListId, op.seriesId, op.toListId);
            tally.applied += 1;
            tally.touched.add(op.fromListId);
            tally.touched.add(op.toListId);
            tally.overview = true;
            return;
        }
      } catch (error) {
        fail(tally, error);
      }
    });
  }

  /** Re-read what the writes touched. Returns the first read failure. */
  async function reconcile(tally: Tally): Promise<unknown> {
    let readError: unknown = null;
    const reads: Promise<void>[] = [];
    for (const listId of tally.touched) {
      if (!state.details[listId]) continue;
      reads.push(
        withLocks([listId], async () => {
          const detail = await api.detail(listId);
          setDetail(detail, true);
        }).catch((error: unknown) => {
          readError ??= error;
        }),
      );
    }
    if (tally.overview && state.lists) {
      reads.push(
        api
          .list()
          .then((lists) => update({ lists: sortLists(lists) }))
          .catch((error: unknown) => {
            readError ??= error;
          }),
      );
    }
    await Promise.all(reads);
    return readError;
  }

  async function perform(
    ops: readonly ReadingListOp[],
    key: string | null,
    meta: Meta | undefined,
  ): Promise<ReadingListOutcome> {
    const tally: Tally = {
      applied: 0,
      failed: 0,
      error: null,
      created: null,
      touched: new Set(),
      overview: false,
    };
    setPending(key, true);
    try {
      for (const op of ops) await execute(op, tally);
      const readError = tally.applied > 0 ? await reconcile(tally) : null;
      const outcome: ReadingListOutcome = {
        ok: tally.failed === 0,
        applied: tally.applied,
        failed: tally.failed,
        error: tally.error,
        created: tally.created,
      };
      setPending(key, false);
      if (disposed) return outcome;
      if (tally.failed > 0) options.onError?.(tally.error, outcome, meta);
      else if (readError) options.onError?.(readError, outcome, meta);
      if (tally.applied > 0) options.onSuccess?.(outcome, meta);
      return outcome;
    } finally {
      if (key) inFlight.delete(key);
    }
  }

  function start(
    ops: readonly ReadingListOp[],
    key: string | null,
    meta: Meta | undefined,
  ): Promise<ReadingListOutcome> {
    if (key) {
      const running = inFlight.get(key);
      if (running) return running;
    }
    const promise = perform(ops, key, meta);
    if (key) inFlight.set(key, promise);
    return promise;
  }

  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    getState: () => state,
    pending: (key) => state.pending.has(key),
    hydrate(data) {
      const patch: { -readonly [K in keyof ReadingListsSnapshot]?: ReadingListsSnapshot[K] } = {};
      if (data.lists) patch.lists = sortLists(data.lists);
      if (data.detail) {
        confirmedOrder.set(data.detail.id, data.detail.items.map((item) => item.id));
        patch.details = { ...state.details, [data.detail.id]: data.detail };
      }
      update(patch);
    },
    run: (op, meta) => start([op], opKey(op), meta),
    runAll: (ops, meta) => start(ops, ops.length > 0 ? "batch" : null, meta),
    dispose() {
      disposed = true;
      listeners.clear();
      inFlight.clear();
    },
  };
}
