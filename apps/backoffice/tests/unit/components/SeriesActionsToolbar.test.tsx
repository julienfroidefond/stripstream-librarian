import type { ComponentProps } from "react";

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

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

const baseProps: ComponentProps<typeof SeriesActionsToolbar> = {
  libraryId: "library-1",
  seriesId: "series-1",
  seriesName: "Series",
  bookCount: 3,
  booksReadCount: 1,
  hasActiveUser: false,
  editAuthors: [],
  editGenres: [],
  editPublishers: [],
  editBookAuthor: null,
  editBookLanguage: null,
  editDescription: null,
  editStartYear: null,
  editTotalVolumes: null,
  editStatus: null,
  editLockedFields: {},
  existingLink: null,
  metadataLinks: [],
  missingData: null,
  hiddenProviders: [],
  readingStatusProvider: null,
  readingStatusLink: null,
  prowlarrConfigured: false,
  qbConfigured: false,
  telegramEnabled: false,
  ddlProviderEnabled: false,
  renameFormat: null,
  renameFormatHs: null,
};

function menu(): HTMLElement {
  return document.querySelector('[role="menu"]') as HTMLElement;
}

describe("SeriesActionsToolbar", () => {
  it("shows the refresh button outside the More menu and refreshes the current series", async () => {
    render(<SeriesActionsToolbar {...baseProps} />);

    const refreshButtons = screen.getAllByRole("button", { name: "common.refresh" });
    expect(refreshButtons).toHaveLength(1);
    expect(menu().contains(refreshButtons[0])).toBe(false);

    fireEvent.click(refreshButtons[0]);

    await waitFor(() => expect(refreshSeriesByIdAction).toHaveBeenCalledWith("series-1"));
    await waitFor(() => expect(refreshMock).toHaveBeenCalledTimes(1));
  });

  it("keeps the refresh entry in the More menu and refreshes the current series", async () => {
    render(<SeriesActionsToolbar {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "common.more" }));

    const menuItem = screen.getByRole("menuitem", { name: "common.refresh" });
    expect(menu().contains(menuItem)).toBe(true);

    fireEvent.click(menuItem);

    await waitFor(() => expect(refreshSeriesByIdAction).toHaveBeenCalledWith("series-1"));
    await waitFor(() => expect(refreshMock).toHaveBeenCalledTimes(1));
  });
});
