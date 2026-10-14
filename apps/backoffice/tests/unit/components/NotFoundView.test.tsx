import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/i18n/server", () => ({
  getServerTranslations: async () => ({ t: (key: string) => key, locale: "en", setLocale: vi.fn() }),
}));

import { NotFoundView } from "@/app/components/NotFoundView";

describe("NotFoundView", () => {
  it("shows the 404 copy and a link back to the dashboard", async () => {
    render(await NotFoundView());

    expect(screen.getByText("404")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("notFound.title");
    expect(screen.getByText("notFound.description")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "notFound.backHome" })).toHaveAttribute("href", "/");
  });

  it("hides the decorative 404 marker from assistive tech", async () => {
    render(await NotFoundView());

    expect(screen.getByText("404")).toHaveAttribute("aria-hidden", "true");
  });
});
