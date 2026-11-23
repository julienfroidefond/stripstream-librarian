import { describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  telegramDownloadBook: vi.fn(async (_id: string): Promise<void> => {}),
  telegramDeleteBook: vi.fn(async (_id: string, _hard?: boolean): Promise<void> => {}),
  ddlStartDownload: vi.fn(async (_provider: string, _body: unknown): Promise<void> => {}),
  ddlDismissDownload: vi.fn(async (_id: string, _hard?: boolean): Promise<void> => {}),
}));

vi.mock("@/lib/api/browser", () => ({
  browserApi: {
    telegram: {
      downloadBook: api.telegramDownloadBook,
      deleteBook: api.telegramDeleteBook,
    },
    ddl: {
      startDownload: api.ddlStartDownload,
      dismissDownload: api.ddlDismissDownload,
    },
  },
}));

import {
  UnsupportedDownloadActionError,
  type DdlStartContext,
  type DownloadActionContext,
} from "@/lib/downloads/actions";
import {
  createSearchActionStore,
  externalIdOf,
  optimisticStatus,
  searchKey,
  type SearchActionRoute,
  type SearchResultAction,
  type SearchRouteLookup,
} from "@/lib/downloads/searchActions";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** Let the queued microtasks (route calls, queue tails) run. */
function flush(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function ddlStartContext(): DdlStartContext {
  return {
    provider: "annas",
    body: {
      external_id: "file-1",
      filename: "Berserk T01.cbz",
      size: 42,
      library_id: "lib-1",
      series_name: "Berserk",
      volume_number: 1,
    },
  };
}

function start(source: "telegram" | "ddl"): SearchResultAction {
  return { source, verb: "start" };
}

function dismiss(source: "telegram" | "ddl"): SearchResultAction {
  return { source, verb: "dismiss" };
}

/** A store wired to one injectable route, so the protocol is tested alone. */
function harness() {
  const run = vi.fn(async (_id: string, _context?: DownloadActionContext): Promise<void> => {});
  const lookup = vi.fn((_action: SearchResultAction): SearchActionRoute | null => ({ run }));
  const onError = vi.fn();
  const store = createSearchActionStore({ route: lookup, onError });
  return { store, run, lookup, onError };
}

describe("search result identity", () => {
  it("namespaces the provider so the same external id stays two rows", () => {
    expect(searchKey("telegram", "book-1")).toBe("telegram:book-1");
    expect(searchKey("annas", "book-1")).toBe("annas:book-1");
    expect(searchKey("annas", "book-1")).not.toBe(searchKey("telegram", "book-1"));
  });

  it("splits the external id on the first separator only", () => {
    expect(externalIdOf("annas:file:1")).toBe("file:1");
    expect(externalIdOf("telegram:book-1")).toBe("book-1");
    expect(externalIdOf("no-separator")).toBe("no-separator");
  });

  it("derives the optimistic badge from the verb", () => {
    expect(optimisticStatus("start")).toBe("queued");
    expect(optimisticStatus("dismiss")).toBe("dismissed");
  });
});

describe("createSearchActionStore optimistic overlay", () => {
  it("starts idle", () => {
    const { store } = harness();

    expect(store.status("telegram:book-1")).toBeNull();
    expect(store.pending("telegram:book-1")).toBe(false);
    expect(store.version()).toBe(0);
  });

  it("writes the badge synchronously, before the request even starts", async () => {
    const request = deferred<void>();
    const { store, run } = harness();
    run.mockImplementationOnce(() => request.promise);
    const key = searchKey("telegram", "book-1");

    const result = store.run(start("telegram"), key);

    expect(store.status(key)).toBe("queued");
    expect(store.pending(key)).toBe(true);
    expect(run).not.toHaveBeenCalled();

    await flush();
    expect(run).toHaveBeenCalledWith("book-1", undefined);

    request.resolve();
    await expect(result).resolves.toBe(true);
    // The server read may lag: the accepted badge stays until `clear`/`reset`.
    expect(store.status(key)).toBe("queued");
    expect(store.pending(key)).toBe(false);
  });

  it("addresses a Telegram row by its external id", async () => {
    const { store, run, lookup } = harness();

    await store.run(start("telegram"), searchKey("telegram", "book-1"));

    expect(lookup).toHaveBeenCalledWith(start("telegram"));
    expect(run).toHaveBeenCalledWith("book-1", undefined);
  });

  it("addresses a DDL start with the release payload and the external id", async () => {
    const { store, run } = harness();
    const context = ddlStartContext();

    await store.run(start("ddl"), searchKey("annas", "file-1"), { ddlStart: context });

    expect(run).toHaveBeenCalledWith("file-1", { ddlStart: context });
  });

  it("addresses a DDL dismiss by its download row, not by the release key", async () => {
    const { store, run } = harness();

    await store.run(dismiss("ddl"), searchKey("annas", "file-1"), { downloadId: "dl-9" });

    expect(run).toHaveBeenCalledWith("dl-9", undefined);
  });
});

describe("createSearchActionStore queue", () => {
  it("serialises writes on one key, newest badge winning", async () => {
    const first = deferred<void>();
    const { store, run } = harness();
    run.mockImplementationOnce(() => first.promise);
    const key = searchKey("telegram", "book-1");

    const started = store.run(start("telegram"), key);
    const dismissed = store.run(dismiss("telegram"), key);

    await flush();
    expect(run).toHaveBeenCalledTimes(1);
    expect(store.status(key)).toBe("dismissed");

    first.resolve();
    await flush();
    expect(run).toHaveBeenCalledTimes(2);

    await expect(Promise.all([started, dismissed])).resolves.toEqual([true, true]);
    expect(store.pending(key)).toBe(false);
  });

  it("runs distinct keys in parallel", async () => {
    const first = deferred<void>();
    const second = deferred<void>();
    const { store, run } = harness();
    run.mockImplementationOnce(() => first.promise).mockImplementationOnce(() => second.promise);
    const telegramKey = searchKey("telegram", "book-1");
    const ddlKey = searchKey("annas", "file-1");

    const telegramWrite = store.run(start("telegram"), telegramKey);
    const ddlWrite = store.run(start("ddl"), ddlKey, { ddlStart: ddlStartContext() });

    await flush();
    expect(run).toHaveBeenCalledTimes(2);
    expect(store.pending(telegramKey)).toBe(true);
    expect(store.pending(ddlKey)).toBe(true);

    first.resolve();
    await telegramWrite;
    expect(store.pending(telegramKey)).toBe(false);
    expect(store.pending(ddlKey)).toBe(true);

    second.resolve();
    await ddlWrite;
    expect(store.pending(ddlKey)).toBe(false);
  });
});

describe("createSearchActionStore failure policy", () => {
  it("drops the overlay and surfaces the error when nothing was acknowledged", async () => {
    const failure = new Error("boom");
    const { store, run, onError } = harness();
    run.mockRejectedValueOnce(failure);
    const key = searchKey("telegram", "book-1");

    await expect(store.run(start("telegram"), key)).resolves.toBe(false);

    expect(store.status(key)).toBeNull();
    expect(store.pending(key)).toBe(false);
    expect(onError).toHaveBeenCalledWith(failure, start("telegram"), key);
  });

  it("restores the last acknowledged badge instead of dropping it", async () => {
    const { store, run } = harness();
    const key = searchKey("ddl", "file-1");

    await store.run(start("ddl"), key, { ddlStart: ddlStartContext() });
    expect(store.status(key)).toBe("queued");

    run.mockRejectedValueOnce(new Error("dismiss refused"));
    await expect(store.run(dismiss("ddl"), key, { downloadId: "dl-1" })).resolves.toBe(false);

    expect(store.status(key)).toBe("queued");
  });

  it("surfaces a rejected dismiss instead of swallowing it", async () => {
    const failure = new Error("dismiss refused");
    const { store, run, onError } = harness();
    run.mockRejectedValueOnce(failure);
    const key = searchKey("annas", "file-1");

    await expect(store.run(dismiss("ddl"), key, { downloadId: "dl-9" })).resolves.toBe(false);

    expect(onError).toHaveBeenCalledTimes(1);
    expect(onError).toHaveBeenCalledWith(failure, dismiss("ddl"), key);
    expect(store.status(key)).toBeNull();
  });

  it("does not let an older failure wipe a newer optimistic badge", async () => {
    const first = deferred<void>();
    const second = deferred<void>();
    const { store, run, onError } = harness();
    run.mockImplementationOnce(() => first.promise).mockImplementationOnce(() => second.promise);
    const key = searchKey("telegram", "book-1");

    const older = store.run(start("telegram"), key);
    const newer = store.run(dismiss("telegram"), key);
    expect(store.status(key)).toBe("dismissed");

    first.reject(new Error("older failed"));
    await expect(older).resolves.toBe(false);

    expect(onError).toHaveBeenCalledTimes(1);
    expect(store.status(key)).toBe("dismissed");
    expect(store.pending(key)).toBe(true);

    await flush();
    expect(run).toHaveBeenCalledTimes(2);

    second.resolve();
    await expect(newer).resolves.toBe(true);
    expect(store.status(key)).toBe("dismissed");
  });

  it("keeps the queue alive when the error handler throws", async () => {
    const onError = vi.fn(() => {
      throw new Error("toast broke");
    });
    const run = vi.fn(async () => {
      throw new Error("write failed");
    });
    const store = createSearchActionStore({ route: () => ({ run }), onError });
    const key = searchKey("telegram", "book-1");

    await expect(store.run(start("telegram"), key)).resolves.toBe(false);
    await expect(store.run(start("telegram"), key)).resolves.toBe(false);

    expect(run).toHaveBeenCalledTimes(2);
    expect(store.pending(key)).toBe(false);
  });

  it("reports an unsupported pair instead of writing", async () => {
    const lookup = vi.fn((_action: SearchResultAction): SearchActionRoute | null => null);
    const onError = vi.fn();
    const store = createSearchActionStore({ route: lookup, onError });
    const key = searchKey("telegram", "book-1");

    await expect(store.run(start("telegram"), key)).resolves.toBe(false);

    expect(onError).toHaveBeenCalledTimes(1);
    expect(onError.mock.calls[0][0]).toBeInstanceOf(UnsupportedDownloadActionError);
    expect(store.status(key)).toBeNull();
  });

  it("refuses a DDL write without its addressing context", async () => {
    const { store, run, onError } = harness();
    const key = searchKey("annas", "file-1");

    await expect(store.run(start("ddl"), key)).resolves.toBe(false);
    await expect(store.run(dismiss("ddl"), key)).resolves.toBe(false);

    expect(run).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledTimes(2);
    expect(String(onError.mock.calls[0][0])).toContain("ddlStart");
    expect(String(onError.mock.calls[1][0])).toContain("downloadId");
    expect(store.status(key)).toBeNull();
  });
});

describe("createSearchActionStore overlay clearing", () => {
  it("clears acknowledged overlays but keeps the one still in flight", async () => {
    const request = deferred<void>();
    const { store, run } = harness();
    run.mockImplementationOnce(() => request.promise);
    const inFlightKey = searchKey("telegram", "in-flight");
    const doneKey = searchKey("telegram", "done");

    const inFlight = store.run(start("telegram"), inFlightKey);
    await store.run(start("telegram"), doneKey);
    expect(store.status(doneKey)).toBe("queued");

    store.clear([inFlightKey, doneKey]);

    expect(store.status(inFlightKey)).toBe("queued");
    expect(store.status(doneKey)).toBeNull();

    request.resolve();
    await inFlight;
  });

  it("drops every overlay on reset and stops an in-flight write from writing back", async () => {
    const request = deferred<void>();
    const { store, run } = harness();
    run.mockImplementationOnce(() => request.promise);
    const key = searchKey("telegram", "book-1");

    const inFlight = store.run(start("telegram"), key);
    await flush();
    expect(store.status(key)).toBe("queued");

    store.reset();
    expect(store.status(key)).toBeNull();
    expect(store.pending(key)).toBe(true);

    request.resolve();
    await expect(inFlight).resolves.toBe(true);

    expect(store.status(key)).toBeNull();
    expect(store.pending(key)).toBe(false);
  });

  it("still surfaces a failure that lands after a reset", async () => {
    const request = deferred<void>();
    const { store, run, onError } = harness();
    run.mockImplementationOnce(() => request.promise);
    const key = searchKey("telegram", "book-1");

    const inFlight = store.run(start("telegram"), key);
    await flush();
    store.reset();

    request.reject(new Error("late failure"));
    await expect(inFlight).resolves.toBe(false);

    expect(onError).toHaveBeenCalledTimes(1);
    expect(store.status(key)).toBeNull();
  });
});

describe("createSearchActionStore subscription", () => {
  it("notifies on each state change and stops after unsubscribe", async () => {
    const listener = vi.fn();
    const { store } = harness();
    const unsubscribe = store.subscribe(listener);
    const before = store.version();

    await store.run(start("telegram"), searchKey("telegram", "book-1"));

    expect(store.version()).toBeGreaterThan(before);
    const changes = listener.mock.calls.length;
    expect(changes).toBeGreaterThan(0);

    unsubscribe();
    await store.run(start("telegram"), searchKey("telegram", "book-2"));

    expect(listener).toHaveBeenCalledTimes(changes);
  });

  it("drops its state and stops notifying once disposed", async () => {
    const listener = vi.fn();
    const { store } = harness();
    store.subscribe(listener);
    const key = searchKey("telegram", "book-1");
    await store.run(start("telegram"), key);
    listener.mockClear();

    store.dispose();

    expect(store.status(key)).toBeNull();
    expect(store.pending(key)).toBe(false);

    await store.run(start("telegram"), key);
    expect(listener).not.toHaveBeenCalled();
  });
});

describe("default route table", () => {
  it("reuses the acquisition routes of the downloads page", async () => {
    const store = createSearchActionStore();
    const context = ddlStartContext();

    await store.run(start("telegram"), searchKey("telegram", "book-1"));
    expect(api.telegramDownloadBook).toHaveBeenCalledWith("book-1");

    await store.run(dismiss("telegram"), searchKey("telegram", "book-2"));
    expect(api.telegramDeleteBook).toHaveBeenCalledWith("book-2", false);

    await store.run(start("ddl"), searchKey("annas", "file-1"), { ddlStart: context });
    expect(api.ddlStartDownload).toHaveBeenCalledWith("annas", context.body);

    await store.run(dismiss("ddl"), searchKey("annas", "file-1"), { downloadId: "dl-1" });
    expect(api.ddlDismissDownload).toHaveBeenCalledWith("dl-1", false);
  });
});
