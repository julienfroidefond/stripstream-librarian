import { describe, expect, it, vi } from "vitest";

import {
  createSettingsDraft,
  type SettingsDraftOptions,
  type SettingsDraftSaveResult,
} from "@/lib/settingsDraft";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function makeDraft(
  overrides: Partial<SettingsDraftOptions> = {},
) {
  const write = vi.fn(async (_key: string, value: unknown) => value);
  const onResult = vi.fn((_key: string, _result: SettingsDraftSaveResult) => undefined);
  const validate = vi.fn((_key: string, _value: unknown, _acknowledged: unknown): string | null => null);
  const draft = createSettingsDraft({
    initial: { limits: { concurrent: 2, timeout: 12 }, rename_format: "default" },
    write,
    validate,
    onResult,
    ...overrides,
  });
  return { draft, write, onResult, validate };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("createSettingsDraft", () => {
  it("starts clean and exposes the seeded values", () => {
    const { draft } = makeDraft();

    expect(draft.get("limits")).toEqual({ concurrent: 2, timeout: 12 });
    expect(draft.get("rename_format")).toBe("default");
    expect(draft.getSnapshot()).toMatchObject({
      dirtyKeys: [],
      savingKeys: [],
      errors: {},
    });
  });

  it("merges object patches and replaces scalar sections", () => {
    const { draft, onResult } = makeDraft();

    draft.patch("limits", { concurrent: 4 });
    draft.set("rename_format", "{series_name}");

    expect(draft.get("limits")).toEqual({ concurrent: 4, timeout: 12 });
    expect(draft.get("rename_format")).toBe("{series_name}");
    expect([...draft.getSnapshot().dirtyKeys].sort()).toEqual(["limits", "rename_format"]);
    expect(onResult).not.toHaveBeenCalled();
  });

  it("resets a dirty section to its acknowledged value", () => {
    const { draft } = makeDraft();

    draft.patch("limits", { concurrent: 8 });
    draft.reset("limits");

    expect(draft.get("limits")).toEqual({ concurrent: 2, timeout: 12 });
    expect(draft.getSnapshot().dirtyKeys).toEqual([]);
  });

  it("saves a whole section, adopts the server acknowledgement and clears dirty", async () => {
    const { draft, write, onResult } = makeDraft();
    write.mockResolvedValueOnce({ concurrent: 4, timeout: 12, server_default: true });

    draft.patch("limits", { concurrent: 4 });
    const result = await draft.save("limits");

    expect(result).toEqual<SettingsDraftSaveResult>({ status: "saved" });
    expect(write).toHaveBeenCalledWith("limits", { concurrent: 4, timeout: 12 });
    expect(draft.get("limits")).toEqual({ concurrent: 4, timeout: 12, server_default: true });
    expect(draft.getSnapshot().dirtyKeys).toEqual([]);
    expect(draft.getSnapshot().savingKeys).toEqual([]);
    expect(onResult).toHaveBeenCalledWith("limits", { status: "saved" });
  });

  it("does not write or notify a no-op save", async () => {
    const { draft, write, onResult } = makeDraft();

    const result = await draft.save("limits");

    expect(result).toEqual({ status: "unchanged" });
    expect(write).not.toHaveBeenCalled();
    expect(onResult).not.toHaveBeenCalled();
  });

  it("blocks an invalid section before the request and clears the error on edit", async () => {
    const { draft, write, onResult, validate } = makeDraft();
    validate.mockReturnValue("settings.validationProwlarr");

    draft.patch("limits", { concurrent: 9 });
    const result = await draft.save("limits");

    expect(result).toEqual({
      status: "invalid",
      message: "settings.validationProwlarr",
    });
    expect(write).not.toHaveBeenCalled();
    expect(draft.get("limits")).toEqual({ concurrent: 2, timeout: 12 });
    expect(draft.getSnapshot().dirtyKeys).toEqual([]);
    expect(draft.getSnapshot().errors.limits).toBe("settings.validationProwlarr");
    expect(validate).toHaveBeenCalledWith(
      "limits",
      { concurrent: 9, timeout: 12 },
      { concurrent: 2, timeout: 12 },
    );
    expect(onResult).toHaveBeenCalledWith("limits", {
      status: "invalid",
      message: "settings.validationProwlarr",
    });

    draft.patch("limits", { concurrent: 10 });
    expect(draft.getSnapshot().errors.limits).toBeUndefined();
  });

  it("rolls back a failed save to the acknowledged value", async () => {
    const { draft, write, onResult } = makeDraft();
    write.mockRejectedValueOnce(new Error("upstream rejected the value"));

    draft.patch("limits", { concurrent: 8 });
    const result = await draft.save("limits");

    expect(result).toEqual({
      status: "error",
      message: "upstream rejected the value",
    });
    expect(draft.get("limits")).toEqual({ concurrent: 2, timeout: 12 });
    expect(draft.getSnapshot().dirtyKeys).toEqual([]);
    expect(draft.getSnapshot().errors.limits).toBe("upstream rejected the value");
    expect(onResult).toHaveBeenCalledWith("limits", {
      status: "error",
      message: "upstream rejected the value",
    });
  });

  it("keeps a newer edit when an older request fails", async () => {
    const { draft, write } = makeDraft();
    const first = deferred<unknown>();
    write.mockReturnValueOnce(first.promise);

    draft.patch("limits", { concurrent: 4 });
    const pending = draft.save("limits");
    await flush();
    draft.patch("limits", { concurrent: 6 });

    first.reject(new Error("network down"));
    await pending;

    // The failed payload (4) is no longer the draft value, so the newer edit
    // (6) survives the rollback.
    expect(draft.get("limits")).toEqual({ concurrent: 6, timeout: 12 });
    expect(draft.getSnapshot().dirtyKeys).toEqual(["limits"]);
    expect(draft.getSnapshot().errors.limits).toBe("network down");
    expect(draft.getSnapshot().savingKeys).toEqual([]);
  });

  it("serializes concurrent saves and coalesces them onto the latest edit", async () => {
    const { draft, write } = makeDraft();
    const first = deferred<unknown>();
    write.mockReturnValueOnce(first.promise);

    draft.patch("limits", { concurrent: 4 });
    const saveOne = draft.save("limits");
    await flush();

    draft.patch("limits", { concurrent: 6 });
    const saveTwo = draft.save("limits");

    draft.patch("limits", { concurrent: 8 });
    const saveThree = draft.save("limits");

    first.resolve({ concurrent: 4, timeout: 12 });
    const results = await Promise.all([saveOne, saveTwo, saveThree]);

    expect(results.map((result) => result.status)).toEqual(["saved", "saved", "unchanged"]);
    expect(write).toHaveBeenCalledTimes(2);
    expect(write).toHaveBeenNthCalledWith(1, "limits", { concurrent: 4, timeout: 12 });
    expect(write).toHaveBeenNthCalledWith(2, "limits", { concurrent: 8, timeout: 12 });
    expect(draft.get("limits")).toEqual({ concurrent: 8, timeout: 12 });
  });

  it("adopts a redacted server value as the new baseline", async () => {
    const { draft, write } = makeDraft();
    write.mockResolvedValueOnce({ enabled: true, api_key: "********" });

    draft.patch("alldebrid", { enabled: true, api_key: "real-secret" });
    await draft.save("alldebrid");

    expect(draft.get("alldebrid")).toEqual({ enabled: true, api_key: "********" });
    expect(draft.getSnapshot().dirtyKeys).toEqual([]);
    expect(await draft.save("alldebrid")).toEqual({ status: "unchanged" });
    expect(write).toHaveBeenCalledTimes(1);
  });

  it("caches section snapshots until that section changes", () => {
    const { draft } = makeDraft();
    const before = draft.getSectionSnapshot("limits");

    draft.set("rename_format", "changed");
    expect(draft.getSectionSnapshot("limits")).toBe(before);

    draft.patch("limits", { concurrent: 4 });
    const after = draft.getSectionSnapshot("limits");
    expect(after).not.toBe(before);
    expect(after.value).toEqual({ concurrent: 4, timeout: 12 });
    expect(after.dirty).toBe(true);
  });

  it("updates saving state while a request is in flight", async () => {
    const { draft, write } = makeDraft();
    const pending = deferred<unknown>();
    write.mockReturnValueOnce(pending.promise);

    draft.patch("limits", { concurrent: 4 });
    const saving = draft.save("limits");
    await flush();
    expect(draft.getSnapshot().savingKeys).toEqual(["limits"]);
    expect(draft.getSectionSnapshot("limits").saving).toBe(true);

    pending.resolve({ concurrent: 4, timeout: 12 });
    await saving;

    expect(draft.getSnapshot().savingKeys).toEqual([]);
    expect(draft.getSectionSnapshot("limits").saving).toBe(false);
  });

  it("notifies subscribers and stops after dispose", async () => {
    const { draft } = makeDraft();
    const listener = vi.fn();
    draft.subscribe(listener);

    draft.patch("limits", { concurrent: 4 });
    await flush();
    expect(listener).toHaveBeenCalledTimes(1);

    draft.dispose();
    draft.patch("limits", { concurrent: 6 });
    expect(listener).toHaveBeenCalledTimes(1);
  });
});

describe("createSettingsDraft in read-only mode", () => {
  it("is writable by default", () => {
    expect(makeDraft().draft.readOnly).toBe(false);
  });

  it("ignores edits and never writes, so defaults cannot overwrite the stored config", async () => {
    const { draft, write, onResult } = makeDraft({ readOnly: true });
    const listener = vi.fn();
    draft.subscribe(listener);

    expect(draft.readOnly).toBe(true);
    draft.patch("limits", { concurrent: 8 });
    draft.set("rename_format", "custom");

    expect(draft.get("limits")).toEqual({ concurrent: 2, timeout: 12 });
    expect(draft.get("rename_format")).toBe("default");
    expect(draft.getSnapshot().dirtyKeys).toEqual([]);
    expect(listener).not.toHaveBeenCalled();

    await expect(draft.save("limits")).resolves.toEqual({ status: "readonly" });
    await expect(draft.save("rename_format")).resolves.toEqual({ status: "readonly" });
    await flush();
    expect(write).not.toHaveBeenCalled();
    expect(onResult).not.toHaveBeenCalled();
    expect(draft.getSnapshot().savingKeys).toEqual([]);
  });
});
