import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { JobActions } from "@/app/(app)/jobs/components/JobActions";

const actions = vi.hoisted(() => ({
  triggerRebuild: vi.fn(async () => {}),
  triggerFullRebuild: vi.fn(async () => {}),
  triggerRescan: vi.fn(async () => {}),
  triggerThumbnailsRebuild: vi.fn(async () => {}),
  triggerThumbnailsRegenerate: vi.fn(async () => {}),
  triggerMetadataBatch: vi.fn(async () => {}),
  triggerMetadataRematch: vi.fn(async () => {}),
  triggerMetadataRefresh: vi.fn(async () => {}),
  triggerMetadataRefreshAll: vi.fn(async () => {}),
  triggerReadingStatusMatch: vi.fn(async () => {}),
  triggerReadingStatusPush: vi.fn(async () => {}),
  triggerRatingPull: vi.fn(async () => {}),
  triggerDownloadDetection: vi.fn(async () => {}),
  triggerRssPoll: vi.fn(async () => {}),
  triggerTelegramSync: vi.fn(async () => {}),
  triggerTelegramSyncIncremental: vi.fn(async () => {}),
  triggerDdlSearch: vi.fn(async () => {}),
}));

vi.mock("@/app/(app)/jobs/actions", () => actions);

const LIBRARIES = [
  { id: "lib-1", name: "Main library" },
  { id: "lib-2", name: "Mangas" },
];

function renderActions(overrides: Partial<Parameters<typeof JobActions>[0]> = {}) {
  return render(
    <JobActions
      libraries={LIBRARIES}
      initialSelected={null}
      readingStatus
      prowlarr
      ddl
      telegram
      {...overrides}
    />
  );
}

function openPalette() {
  fireEvent.click(screen.getByRole("button", { name: /jobs\.allActions/ }));
  return screen.getByRole("dialog", { name: "jobs.allActions" });
}

function openLibraryPicker() {
  fireEvent.click(screen.getByRole("button", { name: /jobs\.libraryLabel/ }));
  return screen.getByRole("dialog", { name: "jobs.libraryPickerTitle" });
}

function chooseAllLibraries() {
  openLibraryPicker();
  fireEvent.click(screen.getByRole("checkbox", { name: "jobs.allLibrariesOption" }));
}

describe("JobActions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the compact bar with the library picker, quick actions and palette trigger", () => {
    renderActions();

    expect(screen.getByRole("button", { name: /jobs\.libraryLabel/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "jobs.rebuild" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "jobs.refreshMetadataQuick" })
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "jobs.generateThumbnailsQuick" })
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "jobs.downloadDetectionQuick" })
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "jobs.ddlSearchQuick" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "jobs.telegramSyncQuick" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /jobs\.allActions/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "jobs.rescan" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "jobs.fullRebuild" })).not.toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    const quickOrder = Array.from(
      document.querySelectorAll<HTMLButtonElement>("form button[type='submit'][aria-label]")
    ).map((button) => button.getAttribute("aria-label"));
    expect(quickOrder).toEqual([
      "jobs.rebuild",
      "jobs.refreshMetadataQuick",
      "jobs.generateThumbnailsQuick",
      "jobs.downloadDetectionQuick",
      "jobs.ddlSearchQuick",
      "jobs.telegramSyncQuick",
    ]);
  });

  it("explains what each quick action really does", () => {
    renderActions();

    expect(screen.getByText("jobs.rebuildQuickHint")).toBeInTheDocument();
    expect(screen.getByText("jobs.refreshMetadataQuickHint")).toBeInTheDocument();
    expect(screen.getByText("jobs.generateThumbnailsQuickHint")).toBeInTheDocument();
    expect(screen.getByText("jobs.downloadDetectionQuickHint")).toBeInTheDocument();
    expect(screen.getByText("jobs.ddlSearchQuickHint")).toBeInTheDocument();
    expect(screen.getByText("jobs.telegramSyncQuickHint")).toBeInTheDocument();
  });

  it("warns instead of launching a quick action when no library is selected", () => {
    renderActions();

    fireEvent.click(screen.getByRole("button", { name: "jobs.rebuild" }));

    expect(screen.getByRole("alert")).toHaveTextContent("jobs.noLibraryWarning");
    expect(actions.triggerRebuild).not.toHaveBeenCalled();
  });

  it("launches a quick action once a library is selected", async () => {
    renderActions();
    chooseAllLibraries();

    fireEvent.click(screen.getByRole("button", { name: "jobs.rebuild" }));

    await waitFor(() => expect(actions.triggerRebuild).toHaveBeenCalledTimes(1));
  });

  it("launches the metadata refresh job from its quick action", async () => {
    renderActions();
    chooseAllLibraries();

    fireEvent.click(screen.getByRole("button", { name: "jobs.refreshMetadataQuick" }));

    await waitFor(() => expect(actions.triggerMetadataRefresh).toHaveBeenCalledTimes(1));
  });

  it("opens the palette with every action grouped", () => {
    renderActions();
    const dialog = openPalette();

    expect(within(dialog).getByText("jobs.groupIndexation")).toBeInTheDocument();
    expect(within(dialog).getByText("jobs.groupMetadata")).toBeInTheDocument();
    expect(within(dialog).getByText("jobs.groupTelegram")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: /jobs\.fullRebuild/ })).toBeInTheDocument();
    expect(within(dialog).getByText("jobs.telegramSync")).toBeInTheDocument();
  });

  it("hides integration groups that are not configured", () => {
    renderActions({ readingStatus: false, prowlarr: false, ddl: false, telegram: false });
    const dialog = openPalette();

    expect(within(dialog).queryByText("jobs.groupReadingStatus")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("jobs.groupTelegram")).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: /jobs\.ddlSearch/ })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "jobs.downloadDetectionQuick" })
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "jobs.ddlSearchQuick" })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "jobs.telegramSyncQuick" })
    ).not.toBeInTheDocument();
  });

  it("filters the palette with the search field", () => {
    renderActions();
    const dialog = openPalette();

    fireEvent.change(within(dialog).getByLabelText("jobs.searchActions"), {
      target: { value: "telegram" },
    });

    expect(within(dialog).getByText("jobs.groupTelegram")).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: /^jobs\.rebuild/ })).not.toBeInTheDocument();
    expect(within(dialog).queryByText("jobs.groupIndexation")).not.toBeInTheDocument();
  });

  it("opens the palette with the keyboard shortcut", () => {
    renderActions();

    fireEvent.keyDown(window, { key: "j", metaKey: true });

    expect(screen.getByRole("dialog", { name: "jobs.allActions" })).toBeInTheDocument();
  });

  it("allows selecting several libraries", () => {
    renderActions();
    const picker = openLibraryPicker();

    fireEvent.click(within(picker).getByRole("checkbox", { name: "Main library" }));
    fireEvent.click(within(picker).getByRole("checkbox", { name: "Mangas" }));
    fireEvent.click(within(picker).getByRole("button", { name: "jobs.libraryPickerDone" }));

    expect(screen.getByRole("button", { name: /jobs\.librariesSelected/ })).toBeInTheDocument();

    const dialog = openPalette();
    const inputs = Array.from(
      dialog.querySelectorAll<HTMLInputElement>('input[name="library_id"]')
    );
    expect(inputs.map((input) => input.value)).toEqual(["lib-1", "lib-2"]);
  });

  it("selects every library at once", () => {
    renderActions();
    chooseAllLibraries();

    expect(screen.getByRole("button", { name: /jobs\.allLibrariesOption/ })).toBeInTheDocument();

    const dialog = openPalette();
    const inputs = Array.from(
      dialog.querySelectorAll<HTMLInputElement>('input[name="library_id"]')
    );
    expect(inputs.map((input) => input.value)).toEqual([""]);
  });

  it("warns instead of launching when no library is selected", () => {
    renderActions();
    const dialog = openPalette();

    fireEvent.click(within(dialog).getByRole("button", { name: /^jobs\.rebuild/ }));

    expect(within(dialog).getByRole("alert")).toHaveTextContent("jobs.noLibraryWarning");
    expect(actions.triggerRebuild).not.toHaveBeenCalled();
  });

  it("asks for a specific library on a rematch", () => {
    renderActions();
    chooseAllLibraries();
    const dialog = openPalette();

    fireEvent.click(within(dialog).getByRole("button", { name: /jobs\.rematchMetadata/ }));

    expect(within(dialog).getByRole("alert")).toHaveTextContent("jobs.requiresLibrary");
    expect(within(dialog).queryByText("jobs.confirmActionTitle")).not.toBeInTheDocument();
    expect(actions.triggerMetadataRematch).not.toHaveBeenCalled();
  });

  it("confirms destructive actions before launching them", async () => {
    renderActions();
    chooseAllLibraries();
    const dialog = openPalette();

    fireEvent.click(within(dialog).getByRole("button", { name: /jobs\.fullRebuild/ }));

    expect(within(dialog).getByText("jobs.confirmActionTitle")).toBeInTheDocument();
    expect(actions.triggerFullRebuild).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: "jobs.confirmAction" }));

    await waitFor(() => expect(actions.triggerFullRebuild).toHaveBeenCalledTimes(1));
  });
});
