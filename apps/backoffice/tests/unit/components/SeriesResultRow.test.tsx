import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ResultStatusBadge, SeriesResultRow } from "@/app/(app)/jobs/[id]/components/SeriesResultRow";

describe("SeriesResultRow", () => {
  it("links to the series when both the series and library are known", () => {
    render(
      <SeriesResultRow
        seriesId="series-1"
        libraryId="lib-1"
        seriesName="Frieren"
        tone="border-border"
        badge={<span>ok</span>}
      />
    );

    expect(screen.getByRole("link", { name: "Frieren" })).toHaveAttribute("href", "/series/series-1");
  });

  it("renders plain text when the series id is missing", () => {
    render(
      <SeriesResultRow
        seriesId={null}
        libraryId="lib-1"
        seriesName="Frieren"
        tone="border-border"
        badge={<span>ok</span>}
      />
    );

    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.getByText("Frieren")).toBeInTheDocument();
  });

  it("renders plain text when the library id is missing", () => {
    render(
      <SeriesResultRow
        seriesId="series-1"
        libraryId={null}
        seriesName="Frieren"
        tone="border-border"
        badge={<span>ok</span>}
      />
    );

    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });

  it("renders the badge, children and optional error message", () => {
    render(
      <SeriesResultRow
        seriesId="series-1"
        libraryId="lib-1"
        seriesName="Frieren"
        tone="border-border"
        badge={<span>ok</span>}
        errorMessage="Something broke"
      >
        <p>details</p>
      </SeriesResultRow>
    );

    expect(screen.getByText("ok")).toBeInTheDocument();
    expect(screen.getByText("details")).toBeInTheDocument();
    expect(screen.getByText("Something broke")).toBeInTheDocument();
  });

  it("hides the error paragraph when there is no error", () => {
    render(
      <SeriesResultRow
        seriesId="series-1"
        libraryId="lib-1"
        seriesName="Frieren"
        tone="border-border"
        badge={null}
      />
    );

    expect(screen.queryByText("Something broke")).not.toBeInTheDocument();
  });
});

describe("ResultStatusBadge", () => {
  it("applies the given classes and renders its children", () => {
    render(<ResultStatusBadge className="bg-success/10">done</ResultStatusBadge>);

    const badge = screen.getByText("done");
    expect(badge).toHaveClass("bg-success/10", "rounded-full");
  });
});
