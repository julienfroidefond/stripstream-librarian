import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/app/(app)/settings/orphans/OrphansTable", () => ({
  OrphansTable: ({
    series,
    onDeleted,
  }: {
    series: { id: string }[];
    onDeleted: (ids: string[]) => void;
  }) => (
    <div data-testid="orphans-table">
      <span>{series.length}</span>
      <span data-testid="orphan-ids">{series.map((s) => s.id).join(",")}</span>
      <button type="button" onClick={() => onDeleted(series.map((s) => s.id))}>
        delete-all
      </button>
    </div>
  ),
}));

import { OrphansTab } from "@/app/(app)/settings/components/OrphansTab";
import type { OrphanSeriesPageDto } from "@/lib/api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

function page(overrides: Partial<OrphanSeriesPageDto> = {}): OrphanSeriesPageDto {
  return {
    items: [{ id: "s1" } as OrphanSeriesPageDto["items"][number]],
    total: 1,
    page: 1,
    limit: 50,
    ...overrides,
  };
}

function ok(payload: unknown) {
  return { ok: true, json: async () => payload };
}

describe("OrphansTab", () => {
  it("shows a spinner then the table with the total count", async () => {
    let resolve!: (value: unknown) => void;
    fetchMock.mockReturnValue(new Promise((r) => (resolve = r)));

    render(<OrphansTab />);
    expect(screen.getByText("common.loading")).toBeInTheDocument();

    resolve(ok(page({ total: 3 })));
    await waitFor(() => expect(screen.getByTestId("orphans-table")).toHaveTextContent("1"));
    expect(fetchMock).toHaveBeenCalledWith("/api/series/orphans?page=1&limit=50");
    expect(screen.getByText('settings.orphans.count:{"count":3,"plural":"s"}')).toBeInTheDocument();
  });

  it("shows the empty state when there is no orphan series", async () => {
    fetchMock.mockResolvedValue(ok(page({ items: [], total: 0 })));

    render(<OrphansTab />);

    await waitFor(() => expect(screen.getByText("settings.orphans.empty")).toBeInTheDocument());
    expect(screen.getByText("settings.orphans.emptyHint")).toBeInTheDocument();
  });

  it("shows an error message when loading fails", async () => {
    fetchMock.mockRejectedValue(new Error("boom"));

    render(<OrphansTab />);

    await waitFor(() => expect(screen.getByText("settings.orphans.loadError")).toBeInTheDocument());
  });

  it("paginates and refetches the next page", async () => {
    fetchMock.mockImplementation((url: string) =>
      Promise.resolve(
        ok(
          url.includes("page=2")
            ? page({ items: [{ id: "s2" } as OrphanSeriesPageDto["items"][number]], total: 60, page: 2 })
            : page({ items: [{ id: "s1" } as OrphanSeriesPageDto["items"][number]], total: 60, page: 1 })
        )
      )
    );

    render(<OrphansTab />);
    await waitFor(() =>
      expect(screen.getByText('settings.orphans.pageInfo:{"page":1,"pages":2}')).toBeInTheDocument()
    );

    fireEvent.click(screen.getByText("common.next"));

    await waitFor(() =>
      expect(screen.getByText('settings.orphans.pageInfo:{"page":2,"pages":2}')).toBeInTheDocument()
    );
    expect(screen.getByText("common.next").closest("button")).toBeDisabled();
  });

  it("refetches the current page after a partial deletion", async () => {
    fetchMock.mockResolvedValue(ok(page({ items: [{ id: "s1" } as never, { id: "s2" } as never], total: 2 })));

    render(<OrphansTab />);
    await waitFor(() => expect(screen.getByTestId("orphans-table")).toHaveTextContent("2"));
    fetchMock.mockClear();
    fetchMock.mockResolvedValue(ok(page({ items: [{ id: "s2" } as never], total: 1 })));

    fireEvent.click(screen.getByText("delete-all"));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/series/orphans?page=1&limit=50"));
  });

  it("steps back a page when the last items of a page are deleted", async () => {
    fetchMock.mockImplementation((url: string) =>
      Promise.resolve(
        ok(
          url.includes("page=2")
            ? page({ items: [{ id: "s2" } as never], total: 51, page: 2 })
            : page({ items: [{ id: "s1" } as never], total: 51, page: 1 })
        )
      )
    );

    render(<OrphansTab />);
    await waitFor(() => expect(screen.getByText("common.next")).toBeInTheDocument());
    fireEvent.click(screen.getByText("common.next"));
    await waitFor(() =>
      expect(screen.getByText('settings.orphans.pageInfo:{"page":2,"pages":2}')).toBeInTheDocument()
    );

    fireEvent.click(screen.getByText("delete-all"));

    await waitFor(() =>
      expect(screen.getByText('settings.orphans.pageInfo:{"page":1,"pages":2}')).toBeInTheDocument()
    );
  });

  it("only renders the current page so a bulk delete stays page-scoped", async () => {
    fetchMock.mockImplementation((url: string) =>
      Promise.resolve(
        ok(
          url.includes("page=2")
            ? page({ items: [{ id: "s51" } as never], total: 51, page: 2 })
            : page({
                items: Array.from({ length: 50 }, (_, i) => ({ id: `s${i + 1}` }) as never),
                total: 51,
                page: 1,
              })
        )
      )
    );

    render(<OrphansTab />);
    await waitFor(() =>
      expect(screen.getByText('settings.orphans.pageInfo:{"page":1,"pages":2}')).toBeInTheDocument()
    );
    expect(screen.getByTestId("orphan-ids")).toHaveTextContent("s1,s2,");

    fireEvent.click(screen.getByText("common.next"));

    await waitFor(() =>
      expect(screen.getByText('settings.orphans.pageInfo:{"page":2,"pages":2}')).toBeInTheDocument()
    );
    expect(screen.getByTestId("orphan-ids")).toHaveTextContent(/^s51$/);
  });
});
