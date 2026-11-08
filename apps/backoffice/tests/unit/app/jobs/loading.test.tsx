import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en", setLocale: vi.fn() }),
}));

import JobsLoading from "@/app/(app)/jobs/loading";

function shimmers(element: HTMLElement): number {
  return element.querySelectorAll(".skeleton-shimmer").length;
}

describe("JobsLoading", () => {
  it("announces loading and mirrors the jobs toolbar and table", async () => {
    const { container } = render(await JobsLoading());

    expect(screen.getByText("common.loading")).toBeInTheDocument();
    expect(container.querySelector('[aria-busy="true"]')).not.toBeNull();

    // Title icon + title.
    expect(shimmers(screen.getByTestId("page-header-skeleton"))).toBe(2);

    // Library selector + 4 quick actions + the "all actions" palette button.
    expect(shimmers(screen.getByTestId("loading-job-actions"))).toBe(1 + 4 + 1);

    // 3 filter selects + counter, 8 header cells + 10 rows × 8 cells.
    expect(shimmers(screen.getByTestId("table-card-skeleton"))).toBe(3 + 1 + 8 + 10 * 8);
  });
});
