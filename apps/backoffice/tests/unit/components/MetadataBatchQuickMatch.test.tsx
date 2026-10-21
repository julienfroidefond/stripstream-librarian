import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MetadataBatchQuickMatch } from "@/app/(app)/jobs/[id]/components/MetadataBatchQuickMatch";
import type { MetadataBatchResultDto } from "@/lib/api";

function result(
  overrides: Partial<MetadataBatchResultDto>
): MetadataBatchResultDto {
  return {
    id: "r1",
    series_id: "s1",
    series_name: "Berserk",
    status: "auto_matched",
    provider_used: null,
    fallback_used: false,
    candidates_count: 0,
    best_confidence: null,
    best_candidate_json: null,
    link_id: null,
    error_message: null,
    secondary: null,
    ...overrides,
  };
}

describe("MetadataBatchQuickMatch", () => {
  it("shows the secondary provider and the fallback marker when it won", () => {
    render(
      <MetadataBatchQuickMatch
        libraryId="lib-1"
        results={[
          result({
            provider_used: "bdphile",
            fallback_used: true,
            link_id: "link-1",
          }),
        ]}
      />
    );

    expect(screen.getByText("Bdphile")).toBeInTheDocument();
    expect(screen.getByText("metadata.fallbackUsed")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.autoMatched")).toBeInTheDocument();
  });

  it("keeps showing the secondary provider when the primary also matched", () => {
    render(
      <MetadataBatchQuickMatch
        libraryId="lib-1"
        results={[
          result({
            provider_used: "google_books",
            fallback_used: false,
            link_id: "link-1",
            secondary: {
              status: "auto_matched",
              provider: "bdphile",
              fallback_used: true,
              candidates_count: 1,
              best_confidence: 1,
              best_candidate_json: { title: "Berserk", external_id: "ext-2" },
              link_id: "link-2",
              error_message: null,
            },
          }),
        ]}
      />
    );

    expect(screen.getByText("Google Books")).toBeInTheDocument();
    expect(screen.getByText("Bdphile")).toBeInTheDocument();
    expect(screen.getAllByText("jobDetail.autoMatched")).toHaveLength(2);
    expect(screen.queryByText("metadata.fallbackUsed")).not.toBeInTheDocument();
  });

  it("shows an uncertain secondary outcome on an already linked series", () => {
    render(
      <MetadataBatchQuickMatch
        libraryId="lib-1"
        results={[
          result({
            status: "already_linked",
            secondary: {
              status: "low_confidence",
              provider: "bdphile",
              fallback_used: true,
              candidates_count: 1,
              best_confidence: 0.42,
              best_candidate_json: { title: "Berserk", external_id: "ext-2" },
              link_id: null,
              error_message: "Best confidence: 42%",
            },
          }),
        ]}
      />
    );

    expect(screen.getByText("jobDetail.alreadyLinked")).toBeInTheDocument();
    expect(screen.getByText("Bdphile")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.lowConfidence")).toBeInTheDocument();
  });
});
