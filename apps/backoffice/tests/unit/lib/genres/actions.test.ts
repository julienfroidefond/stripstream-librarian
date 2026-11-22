// @vitest-environment node
import { describe, expect, it, vi } from "vitest";

import {
  createGenreActionStore,
  type GenreActionsApi,
} from "@/lib/genres/actions";

function deferred<T = void>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function makeApi(): GenreActionsApi {
  return {
    rename: vi.fn(async (_oldName: string, _newName: string): Promise<void> => {}),
    remove: vi.fn(async (_name: string): Promise<void> => {}),
    assign: vi.fn(async (_genre: string, _seriesIds: string[]): Promise<void> => {}),
  };
}

describe("createGenreActionStore", () => {
  it("keeps the subject busy from the call until the write settles", async () => {
    const api = makeApi();
    const call = deferred();
    vi.mocked(api.rename).mockReturnValueOnce(call.promise);
    const store = createGenreActionStore({ api });

    const written = store.run({ type: "rename", from: "Action", to: "Aventure" });

    expect(store.busy("Action")).toBe(true);
    // The write acts on the old name: the new one is not a subject yet.
    expect(store.busy("Aventure")).toBe(false);

    call.resolve();
    await expect(written).resolves.toBe(true);

    expect(store.busy("Action")).toBe(false);
    expect(api.rename).toHaveBeenCalledWith("Action", "Aventure");
  });

  it("runs distinct genres in parallel", async () => {
    const api = makeApi();
    const first = deferred();
    const second = deferred();
    vi.mocked(api.remove)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const store = createGenreActionStore({ api });

    const a = store.run({ type: "delete", name: "Action" });
    const b = store.run({ type: "delete", name: "Aventure" });

    await Promise.resolve();
    expect(api.remove).toHaveBeenCalledTimes(2);
    expect(store.busy("Action")).toBe(true);
    expect(store.busy("Aventure")).toBe(true);

    first.resolve();
    await expect(a).resolves.toBe(true);
    expect(store.busy("Action")).toBe(false);
    expect(store.busy("Aventure")).toBe(true);

    second.resolve();
    await expect(b).resolves.toBe(true);
    expect(store.busy("Aventure")).toBe(false);
  });

  it("serialises two different writes on one genre", async () => {
    const api = makeApi();
    const first = deferred();
    vi.mocked(api.rename).mockReturnValueOnce(first.promise);
    vi.mocked(api.remove).mockResolvedValueOnce(undefined);
    const store = createGenreActionStore({ api });

    const rename = store.run({ type: "rename", from: "Action", to: "Aventure" });
    const remove = store.run({ type: "delete", name: "Action" });

    await Promise.resolve();
    expect(api.rename).toHaveBeenCalledTimes(1);
    expect(api.remove).not.toHaveBeenCalled();
    expect(store.busy("Action")).toBe(true);

    first.resolve();
    await expect(rename).resolves.toBe(true);
    await expect(remove).resolves.toBe(true);
    expect(api.remove).toHaveBeenCalledWith("Action");
    expect(store.busy("Action")).toBe(false);
  });

  it("coalesces an identical write onto the in-flight request", async () => {
    const api = makeApi();
    const call = deferred();
    vi.mocked(api.assign).mockReturnValueOnce(call.promise);
    const store = createGenreActionStore({ api });

    const first = store.run({ type: "assign", genre: "Action", seriesIds: ["s2", "s1"] });
    const second = store.run({ type: "assign", genre: "Action", seriesIds: ["s1", "s2"] });

    expect(second).toBe(first);
    await Promise.resolve();
    expect(api.assign).toHaveBeenCalledTimes(1);

    call.resolve();
    await expect(first).resolves.toBe(true);
    await expect(second).resolves.toBe(true);
    expect(api.assign).toHaveBeenCalledWith("Action", ["s2", "s1"]);
  });

  it("reports a failure and still runs the queued write", async () => {
    const api = makeApi();
    const first = deferred();
    const error = new Error("boom");
    vi.mocked(api.rename).mockReturnValueOnce(first.promise);
    vi.mocked(api.remove).mockResolvedValueOnce(undefined);
    const onError = vi.fn();
    const store = createGenreActionStore({ api, onError });

    const failed = store.run({ type: "rename", from: "Action", to: "Aventure" });
    const queued = store.run({ type: "delete", name: "Action" });

    await Promise.resolve();
    first.reject(error);

    await expect(failed).resolves.toBe(false);
    expect(onError).toHaveBeenCalledWith(error, {
      type: "rename",
      from: "Action",
      to: "Aventure",
    });
    await expect(queued).resolves.toBe(true);
    expect(api.remove).toHaveBeenCalledWith("Action");
    expect(store.busy("Action")).toBe(false);
  });

  it("keeps the subject busy until the aftermath settles", async () => {
    const api = makeApi();
    const write = deferred();
    const aftermath = deferred();
    vi.mocked(api.rename).mockReturnValueOnce(write.promise);
    const store = createGenreActionStore({ api });
    const after = vi.fn(() => aftermath.promise);

    const settled = store.run({ type: "rename", from: "Action", to: "Aventure" }, after);
    await Promise.resolve();
    write.resolve();
    await Promise.resolve();

    expect(after).toHaveBeenCalledTimes(1);
    expect(store.busy("Action")).toBe(true);

    aftermath.resolve();
    await expect(settled).resolves.toBe(true);
    expect(store.busy("Action")).toBe(false);
  });

  it("does not run the aftermath when the write fails", async () => {
    const api = makeApi();
    const onError = vi.fn();
    vi.mocked(api.rename).mockRejectedValueOnce(new Error("boom"));
    const store = createGenreActionStore({ api, onError });
    const after = vi.fn();

    await expect(
      store.run({ type: "rename", from: "Action", to: "Aventure" }, after),
    ).resolves.toBe(false);

    expect(after).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledTimes(1);
    expect(store.busy("Action")).toBe(false);
  });

  it("does not turn an accepted write into a failure when the aftermath throws", async () => {
    const api = makeApi();
    vi.mocked(api.rename).mockResolvedValueOnce(undefined);
    const store = createGenreActionStore({ api });
    const after = vi.fn(async () => {
      throw new Error("refresh failed");
    });

    await expect(
      store.run({ type: "rename", from: "Action", to: "Aventure" }, after),
    ).resolves.toBe(true);

    expect(after).toHaveBeenCalledTimes(1);
    expect(store.busy("Action")).toBe(false);
  });

  it("routes an AI suggestion to a one-series assignment under the genre", async () => {
    const api = makeApi();
    vi.mocked(api.assign).mockResolvedValueOnce(undefined);
    const store = createGenreActionStore({ api });

    const written = store.run({
      type: "applySuggestion",
      genre: "Action",
      seriesId: "s1",
    });

    expect(store.busy("Action")).toBe(true);
    expect(store.busy("s1")).toBe(false);
    await expect(written).resolves.toBe(true);
    expect(api.assign).toHaveBeenCalledWith("Action", ["s1"]);
    expect(store.busy("Action")).toBe(false);
  });

  it("notifies subscribers on every busy change", async () => {
    const api = makeApi();
    const call = deferred();
    vi.mocked(api.rename).mockReturnValueOnce(call.promise);
    const store = createGenreActionStore({ api });
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);
    const version = store.version();

    const written = store.run({ type: "rename", from: "Action", to: "Aventure" });
    expect(listener).toHaveBeenCalledTimes(1);
    expect(store.version()).toBeGreaterThan(version);

    call.resolve();
    await expect(written).resolves.toBe(true);
    expect(listener).toHaveBeenCalledTimes(2);

    unsubscribe();
    vi.mocked(api.remove).mockResolvedValueOnce(undefined);
    await store.run({ type: "delete", name: "Aventure" });
    expect(listener).toHaveBeenCalledTimes(2);
  });

  it("hands the API a mutable copy of the series ids", async () => {
    const api = makeApi();
    const seriesIds = ["s1", "s2"] as const;
    let received: string[] | null = null;
    vi.mocked(api.assign).mockImplementationOnce(async (_genre, ids) => {
      received = ids;
    });
    const store = createGenreActionStore({ api });

    await store.run({ type: "assign", genre: "Action", seriesIds });

    expect(received).toEqual(["s1", "s2"]);
    expect(received).not.toBe(seriesIds);
  });
});
