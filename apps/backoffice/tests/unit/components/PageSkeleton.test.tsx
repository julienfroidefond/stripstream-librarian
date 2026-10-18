import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en", setLocale: vi.fn() }),
}));

import { PageSkeleton } from "@/app/components/PageSkeleton";

describe("PageSkeleton", () => {
  it("announces loading and renders a table-shaped placeholder", async () => {
    const { container } = render(await PageSkeleton({ variant: "table", rows: 3 }));

    expect(screen.getByText("common.loading")).toBeInTheDocument();
    expect(container.querySelector('[aria-busy="true"]')).not.toBeNull();
    // 1 title + 3 toolbar blocks + 3 rows × 4 cells
    expect(container.querySelectorAll(".skeleton-shimmer")).toHaveLength(1 + 3 + 3 * 4);
  });

  it("renders one card placeholder per row in grid mode", async () => {
    const { container } = render(await PageSkeleton({ variant: "grid", rows: 5 }));

    // 1 title + 3 toolbar blocks + 5 cards
    expect(container.querySelectorAll(".skeleton-shimmer")).toHaveLength(1 + 3 + 5);
  });
});
