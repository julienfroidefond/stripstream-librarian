// @vitest-environment node

import { redirect } from "next/navigation";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  rebuildIndex: vi.fn(),
  rebuildThumbnails: vi.fn(),
  regenerateThumbnails: vi.fn(),
  startDdlSearchAll: vi.fn(),
  startDownloadDetection: vi.fn(),
  startMetadataBatch: vi.fn(),
  startMetadataRefresh: vi.fn(),
  startMetadataRefreshAll: vi.fn(),
  startReadingStatusMatch: vi.fn(),
  startReadingStatusPull: vi.fn(),
  startReadingStatusPush: vi.fn(),
  startRssPoll: vi.fn(),
  startTelegramSync: vi.fn(),
  startTelegramSyncIncremental: vi.fn(),
}));

vi.mock("@/lib/api", () => api);
vi.mock("next/cache", () => ({ revalidatePath: vi.fn() }));
vi.mock("next/navigation", () => ({ redirect: vi.fn() }));

import { ApiError } from "@/lib/api/errors";
import {
  triggerMetadataRematch,
  triggerReadingStatusPush,
  triggerRebuild,
  triggerTelegramSync,
} from "@/app/(app)/jobs/actions";

function formDataWithLibraries(...libraryIds: string[]) {
  const formData = new FormData();
  for (const libraryId of libraryIds) formData.append("library_id", libraryId);
  return formData;
}

beforeEach(() => {
  vi.clearAllMocks();
  api.rebuildIndex.mockResolvedValue({ id: "job-1" });
  api.startMetadataBatch.mockResolvedValue({ id: "job-2" });
  api.startTelegramSync.mockResolvedValue({ id: "job-3" });
});

describe("jobs server actions", () => {
  it("launches one job per selected library", async () => {
    await triggerRebuild(formDataWithLibraries("lib-1", "lib-2"));

    expect(api.rebuildIndex).toHaveBeenNthCalledWith(1, "lib-1");
    expect(api.rebuildIndex).toHaveBeenNthCalledWith(2, "lib-2");
    expect(redirect).toHaveBeenCalledWith(
      "/jobs?highlight=job-1&library=lib-1&library=lib-2"
    );
  });

  it("lets the API fan out when every library is selected", async () => {
    await triggerRebuild(formDataWithLibraries(""));

    expect(api.rebuildIndex).toHaveBeenCalledTimes(1);
    expect(api.rebuildIndex).toHaveBeenCalledWith(undefined);
    expect(redirect).toHaveBeenCalledWith("/jobs?highlight=job-1");
  });

  it("rejects a rematch without a specific library", async () => {
    await triggerMetadataRematch(formDataWithLibraries(""));

    expect(api.startMetadataBatch).not.toHaveBeenCalled();
    expect(redirect).toHaveBeenCalledWith("/jobs");
  });

  it("runs a rematch on every selected library", async () => {
    await triggerMetadataRematch(formDataWithLibraries("lib-1", "lib-2"));

    expect(api.startMetadataBatch).toHaveBeenNthCalledWith(1, "lib-1", true);
    expect(api.startMetadataBatch).toHaveBeenNthCalledWith(2, "lib-2", true);
    expect(redirect).toHaveBeenCalledWith(
      "/jobs?highlight=job-2&library=lib-1&library=lib-2"
    );
  });

  it("runs global actions once without a library selection", async () => {
    await triggerTelegramSync(formDataWithLibraries("lib-1"));

    expect(api.startTelegramSync).toHaveBeenCalledTimes(1);
    expect(redirect).toHaveBeenCalledWith("/jobs?highlight=job-3");
  });

  it("redirects with the upstream error detail of an ApiError", async () => {
    api.rebuildIndex.mockRejectedValue(
      new ApiError(400, 'API /index/rebuild failed (400): {"error":"library is busy"}', "library is busy"),
    );

    await triggerRebuild(formDataWithLibraries("lib-1"));

    expect(redirect).toHaveBeenCalledWith(`/jobs?error=${encodeURIComponent("library is busy")}`);
  });

  it("falls back to the error message without an upstream detail", async () => {
    api.startReadingStatusPush.mockRejectedValue(new Error("fetch failed"));

    await triggerReadingStatusPush(formDataWithLibraries("lib-1"));

    expect(redirect).toHaveBeenCalledWith(`/jobs?error=${encodeURIComponent("fetch failed")}`);
  });

  it("redirects global actions on failure too", async () => {
    api.startTelegramSync.mockRejectedValue(new ApiError(503, "API /telegram failed (503): down"));

    await triggerTelegramSync(new FormData());

    expect(redirect).toHaveBeenCalledWith(
      `/jobs?error=${encodeURIComponent("API /telegram failed (503): down")}`,
    );
  });

  it("rethrows Next's navigation signal instead of reporting it as an error", async () => {
    const navigation = Object.assign(new Error("NEXT_REDIRECT"), { digest: "NEXT_REDIRECT;replace;/jobs" });
    vi.mocked(redirect).mockImplementationOnce(() => {
      throw navigation;
    });

    await expect(triggerRebuild(formDataWithLibraries("lib-1"))).rejects.toBe(navigation);
    expect(redirect).toHaveBeenCalledTimes(1);
  });
});
