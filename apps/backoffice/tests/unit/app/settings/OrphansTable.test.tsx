import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/app/components/ui/Toast", () => ({ toast: vi.fn() }));

import { toast } from "@/app/components/ui/Toast";
import { OrphansTable } from "@/app/(app)/settings/orphans/OrphansTable";
import type { OrphanSeriesItemDto } from "@/lib/api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  vi.mocked(toast).mockReset();
});

function makeOrphan(overrides: Partial<OrphanSeriesItemDto> = {}): OrphanSeriesItemDto {
  return {
    id: "s1",
    name: "Blacksad",
    library_id: "lib1",
    library_name: "BD",
    metadata_provider: "bdtheque",
    external_url: "https://example.test/blacksad",
    created_at: "2024-03-05T10:00:00.000Z",
    age_days: 12,
    has_wishlist_release: false,
    ...overrides,
  };
}

function ok(payload: unknown) {
  return { ok: true, json: async () => payload };
}

describe("OrphansTable", () => {
  it("renders headers, provider badge, metadata link and age", () => {
    render(<OrphansTable series={[makeOrphan()]} onDeleted={vi.fn()} />);

    expect(screen.getByText("settings.orphans.colName")).toBeInTheDocument();
    expect(screen.getByText("settings.orphans.colLibrary")).toBeInTheDocument();
    expect(screen.getByText("settings.orphans.colProvider")).toBeInTheDocument();
    expect(screen.getByText("settings.orphans.colMetadata")).toBeInTheDocument();
    expect(screen.getByText("settings.orphans.colAge")).toBeInTheDocument();

    expect(screen.getByText("Blacksad")).toBeInTheDocument();
    expect(screen.getByText("BD")).toBeInTheDocument();
    expect(screen.getByText("bdtheque")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "settings.orphans.openMetadata" })).toHaveAttribute(
      "href",
      "https://example.test/blacksad",
    );
    expect(screen.getByText('settings.orphans.ageDays:{"days":12}')).toBeInTheDocument();
  });

  it("shows the download warning badge and falls back to dashes", () => {
    render(
      <OrphansTable
        series={[
          makeOrphan({
            library_name: null,
            metadata_provider: null,
            external_url: null,
            age_days: 0,
            has_wishlist_release: true,
          }),
        ]}
        onDeleted={vi.fn()}
      />
    );

    expect(screen.getByText("settings.orphans.wishlistRelease")).toBeInTheDocument();
    expect(screen.getByText("settings.orphans.ageToday")).toBeInTheDocument();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.getAllByText("—")).toHaveLength(3);
  });

  it("deletes a single series after confirmation", async () => {
    const onDeleted = vi.fn();
    fetchMock.mockResolvedValue(ok({ deleted: 1, skipped: 0, failed: [] }));
    render(<OrphansTable series={[makeOrphan()]} onDeleted={onDeleted} />);

    fireEvent.click(screen.getByText("settings.orphans.delete"));
    expect(screen.getByText("settings.orphans.deleteConfirmTitle")).toBeInTheDocument();

    const confirmButtons = screen.getAllByText("settings.orphans.delete");
    fireEvent.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() => expect(onDeleted).toHaveBeenCalledWith(["s1"]));
    expect(fetchMock).toHaveBeenCalledWith("/api/series/orphans", {
      method: "DELETE",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ ids: ["s1"] }),
    });
    expect(toast).toHaveBeenCalledWith('settings.orphans.deleted:{"count":1,"plural":""}', "success");
  });

  it("warns about a wishlist release in the single-delete modal", () => {
    render(
      <OrphansTable series={[makeOrphan({ has_wishlist_release: true })]} onDeleted={vi.fn()} />
    );

    fireEvent.click(screen.getByText("settings.orphans.delete"));

    expect(screen.getAllByText("settings.orphans.wishlistReleaseWarning")).toHaveLength(1);
  });

  it("bulk-deletes the series listed on this page", async () => {
    const onDeleted = vi.fn();
    fetchMock.mockResolvedValue(ok({ deleted: 2, skipped: 0, failed: [] }));
    render(
      <OrphansTable
        series={[makeOrphan(), makeOrphan({ id: "s2", name: "Thorgal" })]}
        onDeleted={onDeleted}
      />
    );

    fireEvent.click(screen.getByText("settings.orphans.deletePage"));
    expect(screen.getByText("settings.orphans.deletePageConfirmTitle")).toBeInTheDocument();
    expect(screen.getByText('settings.orphans.deletePageConfirm:{"count":2}')).toBeInTheDocument();

    const confirmButtons = screen.getAllByText("settings.orphans.deletePage");
    fireEvent.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() => expect(onDeleted).toHaveBeenCalledWith(["s1", "s2"]));
    expect(toast).toHaveBeenCalledWith('settings.orphans.deleted:{"count":2,"plural":"s"}', "success");
  });

  it("reports a failed deletion and keeps the selection", async () => {
    const onDeleted = vi.fn();
    fetchMock.mockRejectedValue(new Error("boom"));
    render(<OrphansTable series={[makeOrphan()]} onDeleted={onDeleted} />);

    fireEvent.click(screen.getByText("settings.orphans.delete"));
    const confirmButtons = screen.getAllByText("settings.orphans.delete");
    fireEvent.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() =>
      expect(toast).toHaveBeenCalledWith("settings.orphans.deleteError", "error")
    );
    expect(onDeleted).not.toHaveBeenCalled();
  });

  it("refreshes the table and reports the ids that failed on partial failure", async () => {
    const onDeleted = vi.fn();
    fetchMock.mockResolvedValue(ok({ deleted: 1, skipped: 0, failed: ["s2"] }));
    render(
      <OrphansTable
        series={[makeOrphan(), makeOrphan({ id: "s2", name: "Thorgal" })]}
        onDeleted={onDeleted}
      />
    );

    fireEvent.click(screen.getByText("settings.orphans.deletePage"));
    const confirmButtons = screen.getAllByText("settings.orphans.deletePage");
    fireEvent.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() => expect(onDeleted).toHaveBeenCalledWith(["s1"]));
    expect(toast).toHaveBeenCalledWith('settings.orphans.deleted:{"count":1,"plural":""}', "success");
    expect(toast).toHaveBeenCalledWith(
      'settings.orphans.deletePartial:{"count":1,"plural":""}',
      "error"
    );
  });

  it("does not refresh when every deletion fails", async () => {
    const onDeleted = vi.fn();
    fetchMock.mockResolvedValue(ok({ deleted: 0, skipped: 0, failed: ["s1"] }));
    render(<OrphansTable series={[makeOrphan()]} onDeleted={onDeleted} />);

    fireEvent.click(screen.getByText("settings.orphans.delete"));
    const confirmButtons = screen.getAllByText("settings.orphans.delete");
    fireEvent.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() =>
      expect(toast).toHaveBeenCalledWith('settings.orphans.deletePartial:{"count":1,"plural":""}', "error")
    );
    expect(onDeleted).not.toHaveBeenCalled();
  });
});
