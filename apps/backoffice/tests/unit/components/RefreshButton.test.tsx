import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const { refreshMock, refreshBooksAction, refreshSeriesAction, refreshSeriesByIdAction } =
  vi.hoisted(() => ({
    refreshMock: vi.fn(),
    refreshBooksAction: vi.fn(),
    refreshSeriesAction: vi.fn(),
    refreshSeriesByIdAction: vi.fn(),
  }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh: refreshMock }),
}));

vi.mock("@/app/actions/cache", () => ({
  refreshBooksAction,
  refreshSeriesAction,
  refreshSeriesByIdAction,
}));

import { RefreshButton } from "@/app/components/RefreshButton";

describe("RefreshButton", () => {
  it("refreshes the current series and updates the router", async () => {
    render(<RefreshButton target="series-detail" seriesId="series-1" />);

    fireEvent.click(screen.getByRole("button", { name: "common.refresh" }));

    await waitFor(() => expect(refreshSeriesByIdAction).toHaveBeenCalledWith("series-1"));
    await waitFor(() => expect(refreshMock).toHaveBeenCalledTimes(1));
    expect(refreshBooksAction).not.toHaveBeenCalled();
    expect(refreshSeriesAction).not.toHaveBeenCalled();
  });

  it("disables the button and shows a spinner while refreshing", async () => {
    let resolveRefresh!: () => void;
    refreshSeriesByIdAction.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveRefresh = resolve;
        })
    );
    render(<RefreshButton target="series-detail" seriesId="series-1" />);

    const button = screen.getByRole("button", { name: "common.refresh" });
    expect(button).toHaveAttribute("title", "common.refresh");
    expect(button).toHaveAttribute("aria-label", "common.refresh");
    expect(button.querySelector("svg")).toHaveAttribute("aria-hidden", "true");

    fireEvent.click(button);

    const pendingButton = await screen.findByRole("button", { name: "common.refreshing" });
    expect(pendingButton).toBeDisabled();
    expect(pendingButton).toHaveAttribute("title", "common.refreshing");
    expect(pendingButton).toHaveAttribute("aria-label", "common.refreshing");
    expect(pendingButton.querySelector("svg")).toHaveClass("animate-spin");
    expect(screen.getByText("common.refreshing")).toHaveClass("hidden", "sm:inline");

    fireEvent.click(pendingButton);
    expect(refreshSeriesByIdAction).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveRefresh();
    });

    await waitFor(() => expect(refreshMock).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("button", { name: "common.refresh" })).toBeEnabled();
  });

  it("renders custom children instead of the default button", () => {
    render(
      <RefreshButton target="series-detail" seriesId="series-1">
        {(onClick, pending) => (
          <button type="button" onClick={onClick} disabled={pending}>
            custom
          </button>
        )}
      </RefreshButton>
    );

    expect(screen.getByRole("button", { name: "custom" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "common.refresh" })).toBeNull();
  });
});
