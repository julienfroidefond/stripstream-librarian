import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  MetadataBatchReportCard,
  MetadataBatchResultsCard,
  MetadataRefreshChangesCard,
  MetadataRefreshReportCard,
} from "@/app/(app)/jobs/[id]/components/MetadataReportCards";
import type {
  MetadataBatchReportDto,
  MetadataBatchResultDto,
  MetadataRefreshReportDto,
} from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

const t = ((key: string, params?: Record<string, string | number>) =>
  params ? `${key}:${Object.values(params).join(",")}` : key) as unknown as TranslateFunction;

describe("MetadataBatchReportCard", () => {
  it("renders the series count and every stat value", () => {
    const report: MetadataBatchReportDto = {
      job_id: "job-1",
      status: "success",
      total_series: 12,
      processed: 12,
      auto_matched: 5,
      no_results: 2,
      too_many_results: 1,
      low_confidence: 3,
      already_linked: 4,
      errors: 0,
    };

    render(<MetadataBatchReportCard report={report} t={t} />);

    expect(screen.getByText("jobDetail.batchReport")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.seriesAnalyzed:12")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.autoMatched")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.alreadyLinked")).toBeInTheDocument();
    for (const value of ["5", "4", "2", "1", "3", "0"]) {
      expect(screen.getByText(value)).toBeInTheDocument();
    }
  });
});

describe("MetadataBatchResultsCard", () => {
  const results: MetadataBatchResultDto[] = [
    {
      id: "r1",
      series_id: "s1",
      series_name: "Berserk",
      status: "auto_matched",
      provider_used: "anilist",
      fallback_used: true,
      candidates_count: 2,
      best_confidence: 0.734,
      best_candidate_json: { title: "Berserk" },
      link_id: "l1",
      error_message: null,
    },
    {
      id: "r2",
      series_name: "Orpheline",
      status: "error",
      provider_used: null,
      fallback_used: false,
      candidates_count: 0,
      best_confidence: null,
      best_candidate_json: null,
      link_id: null,
      error_message: "boom",
    },
  ];

  it("renders nothing when there are no results", () => {
    const { container } = render(
      <MetadataBatchResultsCard results={[]} libraryId="lib-1" t={t} />
    );
    expect(container.firstChild).toBeNull();
  });

  it("maps statuses to labels and renders provider, candidates and confidence", () => {
    render(<MetadataBatchResultsCard results={results} libraryId="lib-1" t={t} />);

    expect(screen.getByRole("link", { name: "Berserk" })).toHaveAttribute(
      "href",
      "/series/s1"
    );
    expect(screen.getByText("jobDetail.autoMatched")).toBeInTheDocument();
    expect(screen.getByText("common.error")).toBeInTheDocument();
    expect(screen.getByText("anilist metadata.fallbackUsed")).toBeInTheDocument();
    expect(screen.getByText("2 jobDetail.candidates:s")).toBeInTheDocument();
    expect(screen.getByText(/73%/)).toBeInTheDocument();
    expect(screen.getByText("jobDetail.match:Berserk")).toBeInTheDocument();
    expect(screen.getByText("boom")).toBeInTheDocument();
  });
});

describe("MetadataRefreshReportCard", () => {
  it("renders the link count and every stat", () => {
    const report: MetadataRefreshReportDto = {
      job_id: "job-1",
      status: "success",
      total_links: 10,
      refreshed: 4,
      unchanged: 5,
      errors: 1,
      changes: [],
    };

    render(<MetadataRefreshReportCard report={report} t={t} />);

    expect(screen.getByText("jobDetail.refreshReport")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.refreshReportDesc:10")).toBeInTheDocument();
    for (const value of ["4", "5", "1"]) {
      expect(screen.getByText(value)).toBeInTheDocument();
    }
  });
});

describe("MetadataRefreshChangesCard", () => {
  it("renders nothing when there are no changes", () => {
    const report: MetadataRefreshReportDto = {
      job_id: "job-1",
      status: "success",
      total_links: 1,
      refreshed: 1,
      unchanged: 0,
      errors: 0,
      changes: [],
    };
    const { container } = render(
      <MetadataRefreshChangesCard report={report} libraryId="lib-1" t={t} />
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders series and book diffs with old to new values", () => {
    const report: MetadataRefreshReportDto = {
      job_id: "job-1",
      status: "success",
      total_links: 2,
      refreshed: 1,
      unchanged: 0,
      errors: 1,
      changes: [
        {
          series_id: "s1",
          series_name: "Berserk",
          provider: "anilist",
          status: "updated",
          series_changes: [
            { field: "title", old: "Old title", new: "New title" },
            { field: "genres", old: ["action", "drama"], new: ["seinen"] },
          ],
          book_changes: [
            {
              book_id: "b1",
              title: "Vol 1",
              volume: 1,
              changes: [{ field: "title", old: null, new: "Tome 1" }],
            },
          ],
        },
        {
          series_name: "Broken",
          provider: "anilist",
          status: "error",
          series_changes: [],
          book_changes: [],
          error: "provider timeout",
        },
      ],
    };

    render(<MetadataRefreshChangesCard report={report} libraryId="lib-1" t={t} />);

    expect(screen.getByText("jobDetail.refreshChanges")).toBeInTheDocument();
    expect(screen.getAllByText("anilist").length).toBeGreaterThan(0);
    expect(screen.getAllByText("jobDetail.refreshed").length).toBeGreaterThan(0);
    expect(screen.getByText("common.error")).toBeInTheDocument();
    expect(screen.getByText("Old title")).toBeInTheDocument();
    expect(screen.getByText("New title")).toBeInTheDocument();
    expect(screen.getByText("action, drama")).toBeInTheDocument();
    expect(screen.getByText("seinen")).toBeInTheDocument();
    expect(screen.getAllByText("field.title").length).toBeGreaterThan(0);
    expect(screen.getByRole("link", { name: /Vol 1/ })).toHaveAttribute(
      "href",
      "/books/b1"
    );
    expect(screen.getByText("Tome 1")).toBeInTheDocument();
    expect(screen.getByText("provider timeout")).toBeInTheDocument();
  });
});
