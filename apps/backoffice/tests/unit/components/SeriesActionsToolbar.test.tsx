import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AcquisitionReadiness } from "@/lib/acquisition/readiness";
import type { SeriesActionsModel } from "@/lib/series/detail";

const readiness: AcquisitionReadiness = {
  downloads: true,
  prowlarr: false,
  qbittorrent: false,
  alldebrid: false,
  telegram: false,
  ddl: false,
};

const { refreshMock, refreshSeriesByIdAction } = vi.hoisted(() => ({
  refreshMock: vi.fn(),
  refreshSeriesByIdAction: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh: refreshMock }),
}));

vi.mock("next/dynamic", () => ({
  default: () => () => null,
}));

vi.mock("@/app/actions/cache", () => ({
  refreshBooksAction: vi.fn(),
  refreshSeriesAction: vi.fn(),
  refreshSeriesByIdAction,
}));

import { SeriesActionsToolbar } from "@/app/components/SeriesActionsToolbar";

const baseModel: SeriesActionsModel = {
  libraryId: "library-1",
  seriesId: "series-1",
  seriesName: "Series",
  bookCount: 3,
  booksReadCount: 1,
  hasActiveUser: false,
  isShelved: false,
  metadata: null,
  editBookAuthor: null,
  editBookLanguage: null,
  metadataLinks: [],
  existingLink: null,
  missingData: null,
  hiddenProviders: [],
  readingStatusProvider: null,
  readingStatusLink: null,
  readiness,
  ownedVolumes: null,
  renameFormats: { regular: null, hs: null, integral: null, oneshot: null },
};

function menu(): HTMLElement {
  return document.querySelector('[role="menu"]') as HTMLElement;
}

describe("SeriesActionsToolbar", () => {
  it("shows the refresh button outside the More menu and refreshes the current series", async () => {
    render(<SeriesActionsToolbar model={baseModel} />);

    const refreshButtons = screen.getAllByRole("button", { name: "common.refresh" });
    expect(refreshButtons).toHaveLength(1);
    expect(menu().contains(refreshButtons[0])).toBe(false);

    fireEvent.click(refreshButtons[0]);

    await waitFor(() => expect(refreshSeriesByIdAction).toHaveBeenCalledWith("series-1"));
    await waitFor(() => expect(refreshMock).toHaveBeenCalledTimes(1));
  });

  it("keeps the refresh entry in the More menu and refreshes the current series", async () => {
    render(<SeriesActionsToolbar model={baseModel} />);

    fireEvent.click(screen.getByRole("button", { name: "common.more" }));

    const menuItem = screen.getByRole("menuitem", { name: "common.refresh" });
    expect(menu().contains(menuItem)).toBe(true);

    fireEvent.click(menuItem);

    await waitFor(() => expect(refreshSeriesByIdAction).toHaveBeenCalledWith("series-1"));
    await waitFor(() => expect(refreshMock).toHaveBeenCalledTimes(1));
  });

  it("exposes the shelf action when a reader is selected", () => {
    render(<SeriesActionsToolbar model={{ ...baseModel, hasActiveUser: true }} />);

    fireEvent.click(screen.getByRole("button", { name: "common.more" }));

    expect(screen.getByRole("menuitem", { name: "seriesShelf.shelve" })).toBeInTheDocument();
  });

  it("hides the shelf action in admin mode", () => {
    render(<SeriesActionsToolbar model={baseModel} />);

    fireEvent.click(screen.getByRole("button", { name: "common.more" }));

    expect(screen.queryByRole("menuitem", { name: "seriesShelf.shelve" })).not.toBeInTheDocument();
  });
});
