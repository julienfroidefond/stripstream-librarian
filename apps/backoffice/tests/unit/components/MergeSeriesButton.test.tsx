import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const routerMock = vi.hoisted(() => ({ push: vi.fn(), refresh: vi.fn() }));
vi.mock("next/navigation", () => ({ useRouter: () => routerMock }));
vi.mock("@/app/components/ui/Toast", () => ({ toast: vi.fn(), Toaster: () => null }));

const series = vi.hoisted(() => ({ search: vi.fn(), merge: vi.fn() }));
vi.mock("@/lib/api/browser", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api/browser")>();
  return { ...actual, browserApi: { ...actual.browserApi, series } };
});

import { toast } from "@/app/components/ui/Toast";
import { ApiError } from "@/lib/api/errors";
import { MergeSeriesButton } from "@/app/components/MergeSeriesButton";

beforeEach(() => {
  series.search.mockResolvedValue({
    items: [
      { series_id: "self", name: "Astérix", book_count: 3, metadata_provider: null },
      { series_id: "other", name: "Astérix (doublon)", book_count: 2, metadata_provider: null },
    ],
  });
});

afterEach(() => {
  vi.clearAllMocks();
});

async function openAndSelect() {
  render(<MergeSeriesButton seriesId="self" seriesName="Astérix" />);
  fireEvent.click(screen.getByRole("button", { name: /seriesDetail.merge/ }));
  fireEvent.click(await screen.findByText("Astérix (doublon)"));
  fireEvent.click(screen.getByRole("button", { name: "seriesDetail.mergeConfirm" }));
}

describe("MergeSeriesButton", () => {
  it("toasts the merge summary, closes and refreshes on success", async () => {
    series.merge.mockResolvedValue({ books_moved: 2, metadata_moved: 1, downloads_moved: 0 });
    await openAndSelect();

    await vi.waitFor(() =>
      expect(toast).toHaveBeenCalledWith(
        'seriesDetail.mergeSuccess:{"books":2,"metadata":1,"downloads":0}',
        "success",
      ),
    );
    expect(series.merge).toHaveBeenCalledWith("self", "other");
    expect(routerMock.refresh).toHaveBeenCalledTimes(1);
    await vi.waitFor(() =>
      expect(screen.queryByText("seriesDetail.mergeTitle")).not.toBeInTheDocument(),
    );
  });

  it("keeps the dialog open and toasts the upstream error on failure", async () => {
    series.merge.mockRejectedValue(new ApiError(400, "bad", "Cannot merge a series into itself"));
    await openAndSelect();

    await vi.waitFor(() =>
      expect(toast).toHaveBeenCalledWith("Cannot merge a series into itself", "error"),
    );
    expect(screen.getByText("seriesDetail.mergeTitle")).toBeInTheDocument();
    expect(routerMock.refresh).not.toHaveBeenCalled();
  });
});
