import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/app/components/ui", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/app/components/ui")>();
  return { ...actual, toast: vi.fn() };
});

import { toast } from "@/app/components/ui";

import { CopyDiagnosticsButton, PurgeCacheButton } from "@/app/(app)/diagnostics/DiagnosticsActions";

const fetchMock = vi.fn();
const writeText = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  writeText.mockReset();
  writeText.mockResolvedValue(undefined);
  vi.stubGlobal("fetch", fetchMock);
  vi.mocked(toast).mockReset();
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText },
    configurable: true,
  });
});

function ok(payload: unknown) {
  return { ok: true, status: 200, json: async () => payload };
}

describe("PurgeCacheButton", () => {
  it("purges the cache and reports success", async () => {
    fetchMock.mockResolvedValue(ok({ success: true, message: "Cache purged" }));
    render(<PurgeCacheButton />);

    fireEvent.click(screen.getByRole("button", { name: /diagnostics.purgeCache/ }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/settings/cache/clear", { method: "POST" }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("Cache purged", "success"));
  });

  it("reports a purge failure", async () => {
    fetchMock.mockResolvedValue({ ok: false, status: 500, json: async () => ({ error: "boom" }) });
    render(<PurgeCacheButton />);

    fireEvent.click(screen.getByRole("button", { name: /diagnostics.purgeCache/ }));

    await waitFor(() => expect(toast).toHaveBeenCalledWith("boom", "error"));
  });
});

describe("CopyDiagnosticsButton", () => {
  it("copies the pretty-printed payload to the clipboard", async () => {
    const report = { version: "1.2.3", jobs: { stale_pending: 0 } };
    fetchMock.mockResolvedValue(ok(report));
    render(<CopyDiagnosticsButton />);

    fireEvent.click(screen.getByRole("button", { name: /diagnostics.copy/ }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/diagnostics"));
    await waitFor(() =>
      expect(writeText).toHaveBeenCalledWith(JSON.stringify(report, null, 2))
    );
    expect(toast).toHaveBeenCalledWith("diagnostics.copySuccess", "success");
  });

  it("reports a copy failure when the API is unreachable", async () => {
    fetchMock.mockResolvedValue({ ok: false, status: 503, json: async () => ({}) });
    render(<CopyDiagnosticsButton />);

    fireEvent.click(screen.getByRole("button", { name: /diagnostics.copy/ }));

    await waitFor(() => expect(toast).toHaveBeenCalledWith("diagnostics.copyError", "error"));
    expect(writeText).not.toHaveBeenCalled();
  });
});
