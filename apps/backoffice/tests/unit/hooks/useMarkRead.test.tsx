import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { markReadClassName, useMarkRead } from "@/hooks/useMarkRead";

const routerMock = vi.hoisted(() => ({ refresh: vi.fn() }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: vi.fn(),
    replace: vi.fn(),
    refresh: routerMock.refresh,
    back: vi.fn(),
    prefetch: vi.fn(),
  }),
}));

afterEach(() => {
  vi.unstubAllGlobals();
});

function Harness() {
  const { loading, handleClick } = useMarkRead({
    url: "/api/books/book-1/progress",
    method: "PATCH",
    body: { status: "read" },
    errorContext: "Failed to update reading progress",
  });

  return (
    <button type="button" disabled={loading} onClick={handleClick}>
      {loading ? "loading" : "idle"}
    </button>
  );
}

describe("useMarkRead", () => {
  it("posts the body and refreshes the route", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true });
    vi.stubGlobal("fetch", fetchMock);

    render(<Harness />);
    fireEvent.click(screen.getByRole("button"));

    expect(fetchMock).toHaveBeenCalledWith("/api/books/book-1/progress", {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ status: "read" }),
    });
    await vi.waitFor(() => expect(routerMock.refresh).toHaveBeenCalled());
  });

  it("logs the error body but still refreshes when the response is not ok", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const fetchMock = vi.fn().mockResolvedValue({
      ok: false,
      statusText: "Bad Request",
      json: async () => ({ error: "nope" }),
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<Harness />);
    fireEvent.click(screen.getByRole("button"));

    await vi.waitFor(() => expect(errorSpy).toHaveBeenCalledWith("Failed to update reading progress:", "nope"));
    expect(routerMock.refresh).toHaveBeenCalled();

    errorSpy.mockRestore();
  });

  it("logs thrown fetch failures", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("network")));

    render(<Harness />);
    fireEvent.click(screen.getByRole("button"));

    await vi.waitFor(() => expect(errorSpy).toHaveBeenCalledWith("Failed to update reading progress:", expect.any(Error)));

    errorSpy.mockRestore();
  });
});

describe("markReadClassName", () => {
  it("highlights completed items in green", () => {
    expect(markReadClassName(true, false)).toContain("border-green-500/30");
    expect(markReadClassName(false, false)).toContain("border-border");
  });

  it("uses the compact variant when requested", () => {
    expect(markReadClassName(true, true)).toContain("text-green-600");
    expect(markReadClassName(true, true)).toContain("px-1.5");
    expect(markReadClassName(false, true)).toContain("text-muted-foreground");
  });
});
