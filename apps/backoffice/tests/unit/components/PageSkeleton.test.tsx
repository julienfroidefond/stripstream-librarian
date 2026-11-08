import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en", setLocale: vi.fn() }),
}));

import { PageSkeleton } from "@/app/components/PageSkeleton";

function shimmers(element: HTMLElement): number {
  return element.querySelectorAll(".skeleton-shimmer").length;
}

describe("PageSkeleton", () => {
  it("announces loading and renders a table-shaped placeholder", async () => {
    const { container } = render(await PageSkeleton({ variant: "table", rows: 3 }));

    expect(screen.getByText("common.loading")).toBeInTheDocument();
    expect(container.querySelector('[aria-busy="true"]')).not.toBeNull();

    // Title icon + title, no actions or subtitle.
    expect(shimmers(screen.getByTestId("page-header-skeleton"))).toBe(2);

    // 4 header cells + 3 rows × 4 cells.
    expect(shimmers(screen.getByTestId("table-card-skeleton"))).toBe(4 + 3 * 4);

    expect(screen.queryByTestId("page-tabs-skeleton")).toBeNull();
    expect(screen.queryByTestId("filter-bar-skeleton")).toBeNull();
    expect(screen.queryByTestId("results-row-skeleton")).toBeNull();
    expect(screen.queryByTestId("cover-grid-skeleton")).toBeNull();
  });

  it("composes tabs, filter bar, results row and cover grid for catalog pages", async () => {
    render(
      await PageSkeleton({
        variant: "grid",
        rows: 5,
        actions: 2,
        tabs: 3,
        filters: 8,
        results: true,
      }),
    );

    // Title icon + title + 2 header actions.
    expect(shimmers(screen.getByTestId("page-header-skeleton"))).toBe(4);

    expect(screen.getByTestId("page-tabs-skeleton").children).toHaveLength(3);

    // Search input + 8 desktop pills + the mobile filter toggle.
    expect(shimmers(screen.getByTestId("filter-bar-skeleton"))).toBe(1 + 8 + 1);

    expect(shimmers(screen.getByTestId("results-row-skeleton"))).toBe(2);
    expect(screen.getByTestId("cover-grid-skeleton").children).toHaveLength(5);
    expect(screen.queryByTestId("table-card-skeleton")).toBeNull();
  });

  it("renders an optional subtitle and custom table columns", async () => {
    render(
      await PageSkeleton({
        rows: 2,
        subtitle: true,
        columns: ["h-4 w-10", "h-4 w-20", "h-4 w-30"],
      }),
    );

    // Title icon + title + subtitle.
    expect(shimmers(screen.getByTestId("page-header-skeleton"))).toBe(3);

    // 3 header cells + 2 rows × 3 cells.
    expect(shimmers(screen.getByTestId("table-card-skeleton"))).toBe(3 + 2 * 3);
  });
});
