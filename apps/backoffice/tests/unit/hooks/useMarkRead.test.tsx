import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/lib/api/errors";
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

beforeEach(() => {
  routerMock.refresh.mockClear();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function Harness({ action }: { action: () => Promise<unknown> }) {
  const { loading, handleClick } = useMarkRead({
    action,
    errorContext: "Failed to update reading progress",
  });

  return (
    <button type="button" disabled={loading} onClick={handleClick}>
      {loading ? "loading" : "idle"}
    </button>
  );
}

describe("useMarkRead", () => {
  it("runs the action and refreshes the route", async () => {
    const action = vi.fn().mockResolvedValue(undefined);

    render(<Harness action={action} />);
    fireEvent.click(screen.getByRole("button"));

    expect(action).toHaveBeenCalledTimes(1);
    await vi.waitFor(() => expect(routerMock.refresh).toHaveBeenCalled());
  });

  it("logs the API detail but still refreshes when the request fails", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const action = vi.fn().mockRejectedValue(new ApiError(400, "nope", "nope"));

    render(<Harness action={action} />);
    fireEvent.click(screen.getByRole("button"));

    await vi.waitFor(() => expect(errorSpy).toHaveBeenCalledWith("Failed to update reading progress:", "nope"));
    expect(routerMock.refresh).toHaveBeenCalled();

    errorSpy.mockRestore();
  });

  it("logs thrown network failures without refreshing", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const action = vi.fn().mockRejectedValue(new Error("network"));

    render(<Harness action={action} />);
    fireEvent.click(screen.getByRole("button"));

    await vi.waitFor(() => expect(errorSpy).toHaveBeenCalledWith("Failed to update reading progress:", expect.any(Error)));
    expect(routerMock.refresh).not.toHaveBeenCalled();

    errorSpy.mockRestore();
  });
});

describe("markReadClassName", () => {
  it("highlights completed items with the success token", () => {
    expect(markReadClassName(true, false)).toContain("border-success/30");
    expect(markReadClassName(false, false)).toContain("border-border");
  });

  it("uses the compact variant when requested", () => {
    expect(markReadClassName(true, true)).toContain("text-success");
    expect(markReadClassName(true, true)).toContain("px-1.5");
    expect(markReadClassName(false, true)).toContain("text-muted-foreground");
  });
});
