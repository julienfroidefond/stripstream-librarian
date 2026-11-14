import { describe, expect, it, vi } from "vitest";

import {
  actionFor,
  createDownloadActionStore,
  DOWNLOAD_TARGETS,
  DOWNLOAD_VERBS,
  IDLE_DOWNLOAD_ACTION_STATE,
  needsConfirmation,
  supportsDownloadAction,
  type DownloadAction,
  type DownloadActionRoute,
  type DownloadActionStore,
} from "@/lib/downloads/actions";
import type { DdlProviderDownloadDto } from "@/lib/api/ddl-providers";

const api = vi.hoisted(() => ({
  torrentsRetry: vi.fn(async (_id: string): Promise<void> => {}),
  torrentsRemove: vi.fn(async (_id: string): Promise<void> => {}),
  alldebridRestart: vi.fn(async (_id: string): Promise<void> => {}),
  ddlStartDownload: vi.fn(async (_id: string, _body: unknown) => ({
    id: "ddl-new",
    status: "queued",
  })),
  ddlDismissDownload: vi.fn(async (_id: string, _hard?: boolean): Promise<void> => {}),
  telegramDownloadBook: vi.fn(async (_id: string): Promise<void> => {}),
  telegramDeleteBook: vi.fn(async (_id: string, _hard?: boolean): Promise<void> => {}),
}));

vi.mock("@/lib/api/browser", () => ({
  browserApi: {
    downloads: {
      torrents: { retry: api.torrentsRetry, remove: api.torrentsRemove },
      alldebrid: { restart: api.alldebridRestart },
    },
    ddl: {
      startDownload: api.ddlStartDownload,
      dismissDownload: api.ddlDismissDownload,
    },
    telegram: {
      downloadBook: api.telegramDownloadBook,
      deleteBook: api.telegramDeleteBook,
    },
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function action(target: DownloadAction["target"], verb: DownloadAction["verb"]): DownloadAction {
  return { target, verb };
}

function routeFor(
  target: DownloadAction["target"],
  verb: DownloadAction["verb"],
): DownloadActionRoute {
  const route = actionFor(target, verb);
  if (!route) throw new Error(`missing route for ${target}/${verb}`);
  return route;
}

function ddlDownload(overrides: Partial<DdlProviderDownloadDto> = {}): DdlProviderDownloadDto {
  return {
    id: "ddl-1",
    provider: "fourtoutici",
    external_id: "ext-1",
    filename: "Berserk T01.cbz",
    file_size: 42,
    bytes_downloaded: 0,
    download_speed: 0,
    eta: 0,
    series_name: "Berserk",
    series_id: "series-1",
    volume_number: 1,
    library_id: "lib-1",
    library_name: "Manga",
    status: "failed",
    error_message: null,
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

/**
 * The adapter gate: data-driven confirmation, run only when accepted. The store
 * itself must never prompt, so these two tests drive the policy from the route.
 */
async function activate(
  store: DownloadActionStore,
  target: DownloadAction["target"],
  verb: DownloadAction["verb"],
  id: string,
  accepted: boolean,
  context?: Parameters<DownloadActionStore["run"]>[2],
) {
  const route = actionFor(target, verb);
  if (!route) return;
  if (route.confirm && !accepted) return;
  await store.run({ target, verb }, id, context);
}

describe("download action vocabulary", () => {
  it("exposes every download surface and verb", () => {
    expect(DOWNLOAD_TARGETS).toEqual([
      "torrent",
      "alldebrid",
      "ddl",
      "telegram",
      "release",
      "entries",
    ]);
    expect(DOWNLOAD_VERBS).toEqual(["retry", "start", "delete", "dismiss"]);
  });

  it("reports unsupported pairs instead of pretending they exist", () => {
    expect(actionFor("torrent", "dismiss")).toBeNull();
    expect(actionFor("alldebrid", "dismiss")).toBeNull();
    expect(supportsDownloadAction("torrent", "dismiss")).toBe(false);
    expect(supportsDownloadAction("ddl", "dismiss")).toBe(true);
  });
});

describe("actionFor routing", () => {
  it("routes torrent retry and delete", async () => {
    await routeFor("torrent", "retry").run("t-1");
    expect(api.torrentsRetry).toHaveBeenCalledWith("t-1");

    await routeFor("torrent", "delete").run("t-1");
    expect(api.torrentsRemove).toHaveBeenCalledWith("t-1");
  });

  it("routes an AllDebrid retry through the magnet restart and its delete through the torrent row", async () => {
    await routeFor("alldebrid", "retry").run("ad-1");
    expect(api.alldebridRestart).toHaveBeenCalledWith("ad-1");

    await routeFor("alldebrid", "delete").run("ad-1");
    expect(api.torrentsRemove).toHaveBeenCalledWith("ad-1");
    expect(api.torrentsRetry).not.toHaveBeenCalled();
  });

  it("replays a DDL release from the row payload", async () => {
    const row = ddlDownload();

    await routeFor("ddl", "retry").run(row.id, { ddlDownload: row });

    expect(api.ddlStartDownload).toHaveBeenCalledWith("fourtoutici", {
      external_id: "ext-1",
      filename: "Berserk T01.cbz",
      size: 42,
      library_id: "lib-1",
      series_name: "Berserk",
      volume_number: 1,
    });
  });

  it("falls back to the filename when a DDL row has no series name", async () => {
    const row = ddlDownload({ series_name: null });

    await routeFor("ddl", "retry").run(row.id, { ddlDownload: row });

    expect(api.ddlStartDownload).toHaveBeenCalledWith(
      "fourtoutici",
      expect.objectContaining({ series_name: "Berserk T01.cbz" }),
    );
  });

  it("separates the DDL dismiss from the destructive delete", async () => {
    await routeFor("ddl", "dismiss").run("ddl-1");
    expect(api.ddlDismissDownload).toHaveBeenLastCalledWith("ddl-1", false);

    await routeFor("ddl", "delete").run("ddl-1");
    expect(api.ddlDismissDownload).toHaveBeenLastCalledWith("ddl-1", true);
  });

  it("routes Telegram retry, dismiss and delete", async () => {
    await routeFor("telegram", "retry").run("tg-1");
    expect(api.telegramDownloadBook).toHaveBeenCalledWith("tg-1");

    await routeFor("telegram", "dismiss").run("tg-1");
    expect(api.telegramDeleteBook).toHaveBeenLastCalledWith("tg-1", false);

    await routeFor("telegram", "delete").run("tg-1");
    expect(api.telegramDeleteBook).toHaveBeenLastCalledWith("tg-1", true);
  });

  it("refuses a DDL retry without the row it replays", async () => {
    const onError = vi.fn();
    const store = createDownloadActionStore({ onError });

    await store.run(action("ddl", "retry"), "ddl-1");

    expect(api.ddlStartDownload).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(
      expect.objectContaining({ message: expect.stringContaining("ddlDownload") }),
      action("ddl", "retry"),
      "ddl-1",
    );
  });

  it("refuses a DDL retry for a different row", async () => {
    const onError = vi.fn();
    const store = createDownloadActionStore({ onError });

    await store.run(action("ddl", "retry"), "ddl-1", { ddlDownload: ddlDownload({ id: "ddl-2" }) });

    expect(api.ddlStartDownload).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(
      expect.objectContaining({ message: expect.stringContaining("mismatch") }),
      action("ddl", "retry"),
      "ddl-1",
    );
  });
});

describe("download action confirmation policy", () => {
  it("asks before destructive removals and never before a retry", () => {
    expect(needsConfirmation("torrent", "delete")).toBe(true);
    expect(needsConfirmation("alldebrid", "delete")).toBe(true);
    expect(needsConfirmation("ddl", "dismiss")).toBe(true);
    expect(needsConfirmation("ddl", "delete")).toBe(true);
    expect(needsConfirmation("telegram", "delete")).toBe(true);
    expect(needsConfirmation("telegram", "dismiss")).toBe(false);
    expect(needsConfirmation("torrent", "retry")).toBe(false);
    expect(needsConfirmation("ddl", "retry")).toBe(false);
    expect(needsConfirmation("telegram", "retry")).toBe(false);
  });

  it("does nothing when the user refuses", async () => {
    const onRefresh = vi.fn();
    const onError = vi.fn();
    const store = createDownloadActionStore({ onRefresh, onError });

    await activate(store, "torrent", "delete", "t-1", false);

    expect(api.torrentsRemove).not.toHaveBeenCalled();
    expect(onRefresh).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
    expect(store.busy("t-1")).toBe(false);
  });

  it("runs the action when the user accepts", async () => {
    const onRefresh = vi.fn();
    const store = createDownloadActionStore({ onRefresh });

    await activate(store, "torrent", "delete", "t-1", true);

    expect(api.torrentsRemove).toHaveBeenCalledWith("t-1");
    expect(onRefresh).toHaveBeenCalledWith(action("torrent", "delete"), "t-1");
  });
});

describe("createDownloadActionStore", () => {
  it("starts idle and shares one stable idle snapshot", () => {
    const store = createDownloadActionStore();

    expect(store.getState("t-1")).toBe(IDLE_DOWNLOAD_ACTION_STATE);
    expect(store.busy("t-1")).toBe(false);
  });

  it("coalesces concurrent clicks on the same item", async () => {
    const request = deferred<void>();
    api.torrentsRetry.mockImplementationOnce(() => request.promise);
    const onRefresh = vi.fn();
    const store = createDownloadActionStore({ onRefresh });

    const first = store.run(action("torrent", "retry"), "t-1");
    const second = store.run(action("torrent", "retry"), "t-1");

    expect(api.torrentsRetry).toHaveBeenCalledTimes(1);
    expect(store.getState("t-1")).toEqual({ status: "pending", verb: "retry" });
    expect(store.busy("t-1")).toBe(true);

    request.resolve();
    await Promise.all([first, second]);

    expect(api.torrentsRetry).toHaveBeenCalledTimes(1);
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(store.getState("t-1")).toBe(IDLE_DOWNLOAD_ACTION_STATE);
  });

  it("coalesces a different verb clicked on the same pending item", async () => {
    const request = deferred<void>();
    api.torrentsRetry.mockImplementationOnce(() => request.promise);
    const store = createDownloadActionStore();

    const retry = store.run(action("torrent", "retry"), "t-1");
    const remove = store.run(action("torrent", "delete"), "t-1");

    expect(api.torrentsRemove).not.toHaveBeenCalled();
    expect(api.torrentsRetry).toHaveBeenCalledTimes(1);

    request.resolve();
    await Promise.all([retry, remove]);

    expect(api.torrentsRemove).not.toHaveBeenCalled();
  });

  it("runs distinct items concurrently", async () => {
    const firstRequest = deferred<void>();
    const secondRequest = deferred<void>();
    api.torrentsRetry
      .mockImplementationOnce(() => firstRequest.promise)
      .mockImplementationOnce(() => secondRequest.promise);
    const onRefresh = vi.fn();
    const store = createDownloadActionStore({ onRefresh });

    const first = store.run(action("torrent", "retry"), "t-1");
    const second = store.run(action("torrent", "retry"), "t-2");

    expect(api.torrentsRetry).toHaveBeenCalledTimes(2);
    expect(store.busy("t-1")).toBe(true);
    expect(store.busy("t-2")).toBe(true);

    firstRequest.resolve();
    await first;

    expect(store.busy("t-1")).toBe(false);
    expect(store.busy("t-2")).toBe(true);
    expect(onRefresh).toHaveBeenCalledWith(action("torrent", "retry"), "t-1");

    secondRequest.resolve();
    await second;

    expect(store.busy("t-2")).toBe(false);
    expect(onRefresh).toHaveBeenCalledWith(action("torrent", "retry"), "t-2");
  });

  it("resets the item, surfaces the error and skips the refresh on failure", async () => {
    const failure = new Error("upstream exploded");
    api.torrentsRemove.mockRejectedValueOnce(failure);
    const onRefresh = vi.fn();
    const onError = vi.fn();
    const store = createDownloadActionStore({ onRefresh, onError });

    await store.run(action("torrent", "delete"), "t-1");

    expect(onError).toHaveBeenCalledWith(failure, action("torrent", "delete"), "t-1");
    expect(onRefresh).not.toHaveBeenCalled();
    expect(store.getState("t-1")).toBe(IDLE_DOWNLOAD_ACTION_STATE);
    expect(store.busy("t-1")).toBe(false);
  });

  it("reports an unsupported action instead of doing nothing silently", async () => {
    const onError = vi.fn();
    const store = createDownloadActionStore({ onError });

    await store.run(action("torrent", "dismiss"), "t-1");

    expect(onError).toHaveBeenCalledTimes(1);
    const [error, surfacedAction, id] = onError.mock.calls[0];
    expect(error).toBeInstanceOf(Error);
    expect(String((error as Error).message)).toContain("dismiss");
    expect(surfacedAction).toEqual(action("torrent", "dismiss"));
    expect(id).toBe("t-1");
    expect(api.torrentsRemove).not.toHaveBeenCalled();
  });

  it("lets the item run again after a failure", async () => {
    api.torrentsRetry.mockRejectedValueOnce(new Error("transient"));
    const store = createDownloadActionStore();

    await store.run(action("torrent", "retry"), "t-1");
    await store.run(action("torrent", "retry"), "t-1");

    expect(api.torrentsRetry).toHaveBeenCalledTimes(2);
  });

  it("notifies subscribers on each state change and stops after unsubscribe", async () => {
    const listener = vi.fn();
    const store = createDownloadActionStore();
    const unsubscribe = store.subscribe(listener);

    await store.run(action("telegram", "retry"), "tg-1");

    expect(listener).toHaveBeenCalledTimes(2);

    unsubscribe();
    await store.run(action("telegram", "retry"), "tg-1");

    expect(listener).toHaveBeenCalledTimes(2);
  });

  it("resets one item or every item", async () => {
    const request = deferred<void>();
    api.torrentsRetry
      .mockImplementationOnce(() => request.promise)
      .mockImplementationOnce(() => request.promise);
    const store = createDownloadActionStore();

    void store.run(action("torrent", "retry"), "t-1");
    void store.run(action("torrent", "retry"), "t-2");

    store.reset("t-2");
    expect(store.busy("t-1")).toBe(true);
    expect(store.busy("t-2")).toBe(false);

    store.reset();
    expect(store.busy("t-1")).toBe(false);

    request.resolve();
  });

  it("stops notifying once disposed", async () => {
    const listener = vi.fn();
    const store = createDownloadActionStore();
    store.subscribe(listener);

    store.dispose();

    expect(store.getState("t-1")).toBe(IDLE_DOWNLOAD_ACTION_STATE);
    expect(listener).not.toHaveBeenCalled();
  });
});
