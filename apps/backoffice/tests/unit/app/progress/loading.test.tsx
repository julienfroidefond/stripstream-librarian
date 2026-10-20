import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en", setLocale: vi.fn() }),
}));

import ProgressLoading from "@/app/(app)/progress/loading";

describe("ProgressLoading", () => {
  it("announces loading and mirrors the progress page layout", async () => {
    const { container } = render(await ProgressLoading());

    expect(screen.getByText("common.loading")).toBeInTheDocument();
    expect(container.querySelector('[aria-busy="true"]')).not.toBeNull();
    expect(screen.getByTestId("loading-header")).toBeInTheDocument();
    expect(screen.getByTestId("loading-user-tabs").children).toHaveLength(3);
    expect(screen.getByTestId("loading-stats").children).toHaveLength(3);
    expect(screen.getByTestId("loading-currently-reading").querySelectorAll(".skeleton-shimmer").length).toBeGreaterThan(0);
    expect(screen.getByTestId("loading-series").querySelectorAll(".skeleton-shimmer").length).toBeGreaterThan(0);
    expect(screen.getByTestId("loading-recent").querySelectorAll(".skeleton-shimmer").length).toBeGreaterThan(0);
    expect(container.querySelector(".animate-spin")).toBeNull();
    expect(screen.queryByText("Chargement…")).not.toBeInTheDocument();
  });
});
