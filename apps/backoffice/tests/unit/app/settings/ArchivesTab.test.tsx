import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/app/(app)/settings/archives/ArchivesTable", () => ({
  ArchivesTable: ({ series }: { series: unknown[] }) => <div data-testid="archives-table">{series.length}</div>,
}));

import { ArchivesTab } from "@/app/(app)/settings/components/ArchivesTab";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

describe("ArchivesTab", () => {
  it("shows a spinner while loading, then the table", async () => {
    let resolve!: (v: unknown) => void;
    fetchMock.mockReturnValue(new Promise((r) => (resolve = r)));
    render(<ArchivesTab />);
    expect(screen.getByText("common.loading")).toBeInTheDocument();
    resolve({ json: async () => [{ id: "1" }, { id: "2" }] });
    await waitFor(() => expect(screen.getByTestId("archives-table")).toHaveTextContent("2"));
  });

  it("pluralises the archived series count", async () => {
    fetchMock.mockResolvedValue({ json: async () => [{ id: "1" }, { id: "2" }] });
    render(<ArchivesTab />);
    await waitFor(() =>
      expect(
        screen.getByText('settings.archives.count:{"count":2,"plural":"s"}')
      ).toBeInTheDocument()
    );
  });

  it("shows the empty state when there is no archived series", async () => {
    fetchMock.mockResolvedValue({ json: async () => [] });
    render(<ArchivesTab />);
    await waitFor(() => expect(screen.getByText("settings.archives.empty")).toBeInTheDocument());
    expect(screen.getByText("settings.archives.emptyHint")).toBeInTheDocument();
  });

  it("falls back to an empty list for non-array payloads", async () => {
    fetchMock.mockResolvedValue({ json: async () => ({ error: "nope" }) });
    render(<ArchivesTab />);
    await waitFor(() => expect(screen.getByText("settings.archives.empty")).toBeInTheDocument());
  });

  it("shows an error message when loading fails", async () => {
    fetchMock.mockRejectedValue(new Error("boom"));
    render(<ArchivesTab />);
    await waitFor(() => expect(screen.getByText("settings.archives.loadError")).toBeInTheDocument());
  });
});
