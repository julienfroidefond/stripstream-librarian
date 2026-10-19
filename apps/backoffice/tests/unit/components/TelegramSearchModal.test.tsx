import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/app/components/ui", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/app/components/ui")>();
  return { ...actual, toast: vi.fn() };
});

import { toast } from "@/app/components/ui";

import { TelegramSearchModal } from "@/app/components/TelegramSearchModal";
import type { TelegramSearchResultDto } from "@/lib/api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
  vi.mocked(toast).mockReset();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function response(payload: unknown, ok = true, status = 200) {
  return { ok, status, json: async () => payload };
}

function result(overrides: Partial<TelegramSearchResultDto> = {}): TelegramSearchResultDto {
  return {
    id: "book-1",
    channel_username: "channel",
    filename: "Series T01.cbz",
    file_size: 1024,
    volume_number: 1,
    series_name: "Series",
    status: "available",
    created_at: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function mockSearch(
  results: TelegramSearchResultDto[],
  download: () => unknown = () => response({})
) {
  fetchMock.mockImplementation(async (url: string) => {
    if (url === "/api/telegram-monitor/live-search") return response(results);
    if (url.endsWith("/download")) return download();
    return response({});
  });
}

function renderAndOpen() {
  render(
    <TelegramSearchModal seriesName="Series" initialEnabled>
      {(open) => <button onClick={open}>open</button>}
    </TelegramSearchModal>
  );
  fireEvent.click(screen.getByRole("button", { name: "open" }));
}

describe("TelegramSearchModal", () => {
  it.each(["failed", "imported"])(
    "retries a %s result through the download endpoint",
    async (status) => {
      mockSearch([result({ id: `${status}-book`, status })]);
      renderAndOpen();

      fireEvent.click(await screen.findByTitle("downloads.retry"));

      await waitFor(() =>
        expect(fetchMock).toHaveBeenCalledWith(
          `/api/telegram-monitor/books/${status}-book/download`,
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({}),
          }
        )
      );
      expect(await screen.findByText("downloads.status.queued")).toBeInTheDocument();
    }
  );

  it("keeps the available actions and hides retry on queued, downloading and dismissed rows", async () => {
    mockSearch([
      result({ id: "failed", filename: "failed.cbz", status: "failed" }),
      result({ id: "imported", filename: "imported.cbz", status: "imported" }),
      result({ id: "available", filename: "available.cbz", status: "available" }),
      result({ id: "queued", filename: "queued.cbz", status: "queued" }),
      result({ id: "downloading", filename: "downloading.cbz", status: "downloading" }),
      result({ id: "dismissed", filename: "dismissed.cbz", status: "dismissed" }),
    ]);
    renderAndOpen();

    await screen.findByText("failed.cbz");

    for (const filename of ["failed.cbz", "imported.cbz"]) {
      const row = screen.getByText(filename).closest("tr");
      expect(row).not.toBeNull();
      expect(within(row as HTMLElement).getByTitle("downloads.retry")).toBeInTheDocument();
    }

    const availableRow = screen.getByText("available.cbz").closest("tr") as HTMLElement;
    expect(within(availableRow).getByTitle("telegramMonitor.download")).toBeInTheDocument();
    expect(within(availableRow).getByTitle("telegramMonitor.dismiss")).toBeInTheDocument();
    expect(within(availableRow).queryByTitle("downloads.retry")).not.toBeInTheDocument();

    for (const filename of ["queued.cbz", "downloading.cbz", "dismissed.cbz"]) {
      const row = screen.getByText(filename).closest("tr");
      expect(row).not.toBeNull();
      expect(within(row as HTMLElement).queryByTitle("downloads.retry")).not.toBeInTheDocument();
    }
  });

  it("downloads and dismisses an available result through the existing actions", async () => {
    mockSearch([
      result({ id: "download-book", filename: "download.cbz", status: "available" }),
      result({ id: "dismiss-book", filename: "dismiss.cbz", status: "available" }),
    ]);
    renderAndOpen();

    const downloadRow = (await screen.findByText("download.cbz")).closest("tr") as HTMLElement;
    fireEvent.click(within(downloadRow).getByTitle("telegramMonitor.download"));
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/telegram-monitor/books/download-book/download", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      })
    );

    const dismissRow = screen.getByText("dismiss.cbz").closest("tr") as HTMLElement;
    fireEvent.click(within(dismissRow).getByTitle("telegramMonitor.dismiss"));
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/telegram-monitor/books/dismiss-book", {
        method: "DELETE",
      })
    );
  });

  it.each([
    ["failed", "downloads.status.error"],
    ["imported", "downloads.status.imported"],
  ])("restores the %s status and reports the error when retry fails", async (status, label) => {
    mockSearch(
      [result({ status })],
      () => response({ error: "boom" }, false, 500)
    );
    renderAndOpen();

    fireEvent.click(await screen.findByTitle("downloads.retry"));

    await waitFor(() => expect(toast).toHaveBeenCalledWith("boom", "error"));
    expect(await screen.findByText(label)).toBeInTheDocument();
    expect(screen.queryByText("downloads.status.queued")).not.toBeInTheDocument();
  });

  it.each([
    ["failed", "downloads.status.error"],
    ["imported", "downloads.status.imported"],
  ])("restores the %s status and reports a network error when retry rejects", async (status, label) => {
    mockSearch(
      [result({ status })],
      () => Promise.reject(new Error("network down"))
    );
    renderAndOpen();

    fireEvent.click(await screen.findByTitle("downloads.retry"));

    await waitFor(() => expect(toast).toHaveBeenCalledWith("Error: network down", "error"));
    expect(await screen.findByText(label)).toBeInTheDocument();
    expect(screen.queryByText("downloads.status.queued")).not.toBeInTheDocument();
  });
});
