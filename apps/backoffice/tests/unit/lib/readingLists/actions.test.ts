import { describe, expect, it, vi } from "vitest";

import type {
  ReadingListDetailDto,
  ReadingListDto,
  ReadingListSeriesDto,
} from "@/lib/api/reading-lists";
import {
  createReadingListsStore,
  movedOrder,
  opKey,
  type ReadingListsApi,
} from "@/lib/readingLists/actions";

function item(id: string, position: number): ReadingListSeriesDto {
  return {
    id,
    name: `Series ${id}`,
    cover_url: null,
    first_book_id: null,
    first_book_updated_at: null,
    provider: null,
    external_id: null,
    external_url: null,
    library_id: "lib-1",
    library_name: "Main",
    position,
    book_count: 1,
    books_read_count: 0,
    is_empty: false,
  };
}

function detail(id: string, ids: string[]): ReadingListDetailDto {
  return {
    id,
    name: `List ${id}`,
    description: null,
    items: ids.map((seriesId, index) => item(seriesId, index)),
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
  };
}

function summary(id: string, name: string, seriesCount = 0): ReadingListDto {
  return {
    id,
    name,
    description: null,
    series_count: seriesCount,
    book_count: 0,
    books_read_count: 0,
    is_incomplete: seriesCount < 2,
    preview_covers: [],
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
  };
}

function fakeApi(overrides: Partial<ReadingListsApi> = {}): ReadingListsApi {
  return {
    list: vi.fn().mockResolvedValue([]),
    detail: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    remove: vi.fn().mockResolvedValue(undefined),
    addSeries: vi.fn().mockResolvedValue(undefined),
    removeSeries: vi.fn().mockResolvedValue(undefined),
    moveSeries: vi.fn().mockResolvedValue(undefined),
    reorder: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function deferred<T = void>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const ids = (d: ReadingListDetailDto | undefined) => d?.items.map((i) => i.id);

describe("reading lists write store", () => {
  it("names the busy key of each operation", () => {
    expect(opKey({ type: "create", name: "x" })).toBe("create");
    expect(opKey({ type: "rename", listId: "l", name: "x", description: null })).toBe("list:l");
    expect(opKey({ type: "delete", listId: "l" })).toBe("list:l");
    expect(opKey({ type: "add", listId: "l", seriesId: "s" })).toBe("item:l:s");
    expect(opKey({ type: "moveToList", fromListId: "l", toListId: "m", seriesId: "s" })).toBe(
      "item:l:s",
    );
    expect(opKey({ type: "move", listId: "l", seriesId: "s", direction: "up" })).toBeNull();
    expect(movedOrder([{ id: "a" }, { id: "b" }], "b", "up")).toEqual(["b", "a"]);
    expect(movedOrder([{ id: "a" }, { id: "b" }], "a", "up")).toBeNull();
  });

  it("shows an added series only once the server confirmed it, from detail()", async () => {
    const add = deferred();
    const reconciled = detail("l1", ["a", "b"]);
    reconciled.items[1].library_name = "Second library";
    const api = fakeApi({
      addSeries: vi.fn(() => add.promise),
      detail: vi.fn().mockResolvedValue(reconciled),
    });
    const onSuccess = vi.fn();
    const onError = vi.fn();
    const store = createReadingListsStore({
      api,
      details: [detail("l1", ["a"])],
      onSuccess,
      onError,
    });

    const run = store.run({ type: "add", listId: "l1", seriesId: "b" });
    expect(store.pending("item:l1:b")).toBe(true);
    expect(ids(store.getState().details.l1)).toEqual(["a"]);

    add.resolve();
    const outcome = await run;

    expect(outcome).toMatchObject({ ok: true, applied: 1, failed: 0 });
    expect(api.detail).toHaveBeenCalledWith("l1");
    expect(store.getState().details.l1.items[1].library_name).toBe("Second library");
    expect(store.pending("item:l1:b")).toBe(false);
    expect(onSuccess).toHaveBeenCalledTimes(1);
    expect(onError).not.toHaveBeenCalled();
  });

  it("keeps the state, reports the error and does not refresh when a write fails", async () => {
    const error = new Error("boom");
    const api = fakeApi({ removeSeries: vi.fn().mockRejectedValue(error) });
    const onSuccess = vi.fn();
    const onError = vi.fn();
    const store = createReadingListsStore({
      api,
      details: [detail("l1", ["a", "b"])],
      onSuccess,
      onError,
    });

    const outcome = await store.run({ type: "remove", listId: "l1", seriesId: "a" });

    expect(outcome).toMatchObject({ ok: false, applied: 0, failed: 1, error });
    expect(ids(store.getState().details.l1)).toEqual(["a", "b"]);
    expect(api.detail).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(error, outcome, undefined);
    expect(onSuccess).not.toHaveBeenCalled();
  });

  it("coalesces a burst of clicks on the same item onto one request", async () => {
    const add = deferred();
    const api = fakeApi({ addSeries: vi.fn(() => add.promise) });
    const store = createReadingListsStore({ api });

    const first = store.run({ type: "add", listId: "l1", seriesId: "a" });
    const second = store.run({ type: "add", listId: "l1", seriesId: "a" });
    add.resolve();

    expect(second).toBe(first);
    await first;
    expect(api.addSeries).toHaveBeenCalledTimes(1);
  });

  it("reorders optimistically and rolls back to the acknowledged order on failure", async () => {
    const reorder = deferred();
    const api = fakeApi({ reorder: vi.fn(() => reorder.promise) });
    const onError = vi.fn();
    const onSuccess = vi.fn();
    const store = createReadingListsStore({
      api,
      details: [detail("l1", ["a", "b", "c"])],
      onError,
      onSuccess,
    });

    const run = store.run({ type: "move", listId: "l1", seriesId: "c", direction: "up" });
    expect(ids(store.getState().details.l1)).toEqual(["a", "c", "b"]);
    expect(store.getState().details.l1.items.map((i) => i.position)).toEqual([0, 1, 2]);

    reorder.reject(new Error("rejected"));
    await run;

    expect(api.reorder).toHaveBeenCalledWith("l1", ["a", "c", "b"]);
    expect(ids(store.getState().details.l1)).toEqual(["a", "b", "c"]);
    expect(onError).toHaveBeenCalledTimes(1);
    expect(onSuccess).not.toHaveBeenCalled();
  });

  it("serialises rapid reorders and lets the latest one decide the final order", async () => {
    const first = deferred();
    const second = deferred();
    const reorder = vi
      .fn()
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);
    const store = createReadingListsStore({
      api: fakeApi({ reorder }),
      details: [detail("l1", ["a", "b", "c"])],
    });

    const one = store.run({ type: "move", listId: "l1", seriesId: "c", direction: "up" });
    const two = store.run({ type: "move", listId: "l1", seriesId: "c", direction: "up" });
    expect(ids(store.getState().details.l1)).toEqual(["c", "a", "b"]);
    // The second request waits for the first one.
    await vi.waitFor(() => expect(reorder).toHaveBeenCalledTimes(1));
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(reorder).toHaveBeenCalledTimes(1);

    // The first fails while the second is queued: no rollback yet.
    first.reject(new Error("first failed"));
    await vi.waitFor(() => expect(reorder).toHaveBeenCalledTimes(2));
    expect(ids(store.getState().details.l1)).toEqual(["c", "a", "b"]);

    second.resolve();
    await Promise.all([one, two]);
    expect(reorder.mock.calls[1]).toEqual(["l1", ["c", "a", "b"]]);
    expect(ids(store.getState().details.l1)).toEqual(["c", "a", "b"]);
  });

  it("rolls back to the order of the last successful reorder", async () => {
    const reorder = vi
      .fn()
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error("second failed"));
    const store = createReadingListsStore({
      api: fakeApi({ reorder }),
      details: [detail("l1", ["a", "b", "c"])],
    });

    await store.run({ type: "move", listId: "l1", seriesId: "c", direction: "up" });
    await store.run({ type: "move", listId: "l1", seriesId: "c", direction: "up" });

    expect(ids(store.getState().details.l1)).toEqual(["a", "c", "b"]);
  });

  it("renames from the server answer and deletes from the overview and the detail", async () => {
    const renamed = { ...summary("l1", "Zeta"), description: "New" };
    const api = fakeApi({ update: vi.fn().mockResolvedValue(renamed) });
    const store = createReadingListsStore({
      api,
      lists: [summary("l1", "Alpha"), summary("l2", "Beta")],
      details: [detail("l1", ["a"])],
    });

    await store.run({ type: "rename", listId: "l1", name: "Zeta", description: "New" });
    expect(store.getState().lists?.map((l) => l.name)).toEqual(["Beta", "Zeta"]);
    expect(store.getState().details.l1).toMatchObject({ name: "Zeta", description: "New" });

    await store.run({ type: "delete", listId: "l1" });
    expect(store.getState().lists?.map((l) => l.id)).toEqual(["l2"]);
    expect(store.getState().details.l1).toBeUndefined();
  });

  it("removes a created list again when none of its series could be added", async () => {
    const api = fakeApi({
      create: vi.fn().mockResolvedValue(summary("new", "New")),
      addSeries: vi.fn().mockRejectedValue(new Error("nope")),
    });
    const onSuccess = vi.fn();
    const onError = vi.fn();
    const store = createReadingListsStore({ api, lists: [], onSuccess, onError });

    const outcome = await store.run({
      type: "create",
      name: "New",
      seriesIds: ["a", "b"],
    });

    expect(api.remove).toHaveBeenCalledWith("new");
    expect(outcome).toMatchObject({ ok: false, applied: 0, created: null });
    expect(store.getState().lists).toEqual([]);
    expect(onError).toHaveBeenCalledTimes(1);
    expect(onSuccess).not.toHaveBeenCalled();
  });

  it("runs a batch as one action: one reconciliation, one error, one success", async () => {
    const reconciled = [summary("l1", "Alpha", 3), summary("l2", "Beta", 1)];
    const api = fakeApi({
      addSeries: vi
        .fn()
        .mockResolvedValueOnce(undefined)
        .mockRejectedValueOnce(new Error("already there")),
      list: vi.fn().mockResolvedValue(reconciled),
    });
    const onSuccess = vi.fn();
    const onError = vi.fn();
    const store = createReadingListsStore({
      api,
      lists: [summary("l1", "Alpha", 2), summary("l2", "Beta", 1)],
      onSuccess,
      onError,
    });

    const run = store.runAll([
      { type: "add", listId: "l1", seriesId: "a" },
      { type: "add", listId: "l2", seriesId: "b" },
      { type: "moveToList", fromListId: "l2", toListId: "l1", seriesId: "c" },
    ]);
    expect(store.pending("batch")).toBe(true);
    const outcome = await run;

    expect(outcome).toMatchObject({ ok: false, applied: 2, failed: 1 });
    expect(api.list).toHaveBeenCalledTimes(1);
    expect(store.getState().lists).toEqual(reconciled);
    expect(onError).toHaveBeenCalledTimes(1);
    expect(onSuccess).toHaveBeenCalledTimes(1);
    expect(store.pending("batch")).toBe(false);
  });

  it("notifies subscribers with a new snapshot only on change", async () => {
    const store = createReadingListsStore({ api: fakeApi(), lists: [] });
    const before = store.getState();
    expect(store.getState()).toBe(before);
    const listener = vi.fn();
    store.subscribe(listener);
    store.hydrate({ lists: [summary("l1", "Alpha")] });
    expect(listener).toHaveBeenCalled();
    expect(store.getState()).not.toBe(before);
  });
});
