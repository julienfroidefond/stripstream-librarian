import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en", setLocale: vi.fn() }),
}));

import MetadataLoading from "@/app/(app)/metadata/loading";

function shimmers(element: HTMLElement): number {
  return element.querySelectorAll(".skeleton-shimmer").length;
}

describe("MetadataLoading", () => {
  it("announces loading and mirrors the metadata page layout", async () => {
    const { container } = render(await MetadataLoading());

    expect(screen.getByText("common.loading")).toBeInTheDocument();
    expect(container.querySelector('[aria-busy="true"]')).not.toBeNull();

    // Title icon + title + subtitle.
    expect(shimmers(screen.getByTestId("page-header-skeleton"))).toBe(3);

    // Series / books tabs.
    expect(screen.getByTestId("page-tabs-skeleton").children).toHaveLength(2);

    // Search input + one filter pill + the mobile filter toggle.
    expect(shimmers(screen.getByTestId("filter-bar-skeleton"))).toBe(1 + 1 + 1);

    // "all" chip + one chip per gap of the current tab.
    expect(screen.getByTestId("loading-gap-chips").children).toHaveLength(8);
    expect(shimmers(screen.getByTestId("loading-result-count"))).toBe(1);

    // 9 header cells + 8 rows × 9 cells.
    expect(shimmers(screen.getByTestId("table-card-skeleton"))).toBe(9 + 8 * 9);
  });
});
