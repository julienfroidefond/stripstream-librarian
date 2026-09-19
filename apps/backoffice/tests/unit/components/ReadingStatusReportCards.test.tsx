import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  ReadingStatusMatchReportCard,
  ReadingStatusMatchResultsCard,
  ReadingStatusPushReportCard,
  ReadingStatusPushResultsCard,
} from "@/app/(app)/jobs/[id]/components/ReadingStatusReportCards";
import type {
  ReadingStatusMatchReportDto,
  ReadingStatusMatchResultDto,
  ReadingStatusPushReportDto,
  ReadingStatusPushResultDto,
} from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

const t = ((key: string, params?: Record<string, string | number>) =>
  params ? `${key}:${Object.values(params).join(",")}` : key) as unknown as TranslateFunction;

describe("ReadingStatusMatchReportCard", () => {
  it("renders the series count and every stat", () => {
    const report: ReadingStatusMatchReportDto = {
      job_id: "job-1",
      status: "success",
      total_series: 8,
      linked: 3,
      already_linked: 2,
      no_results: 1,
      ambiguous: 1,
      errors: 1,
    };

    render(<ReadingStatusMatchReportCard report={report} t={t} />);

    expect(screen.getByText("jobDetail.readingStatusMatchReport")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.seriesAnalyzed:8")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.linked")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.ambiguous")).toBeInTheDocument();
    for (const value of ["3", "2", "1"]) {
      expect(screen.getAllByText(value).length).toBeGreaterThan(0);
    }
  });
});

describe("ReadingStatusMatchResultsCard", () => {
  const results: ReadingStatusMatchResultDto[] = [
    {
      id: "r1",
      series_id: "s1",
      series_name: "Berserk",
      status: "linked",
      anilist_id: 30002,
      anilist_title: "Berserk",
      anilist_url: "https://anilist.co/manga/30002",
      error_message: null,
    },
    {
      id: "r2",
      series_name: "Sans lien",
      status: "linked",
      anilist_id: null,
      anilist_title: "Sans URL",
      anilist_url: null,
      error_message: null,
    },
    {
      id: "r3",
      series_name: "Ambigu",
      status: "ambiguous",
      anilist_id: null,
      anilist_title: null,
      anilist_url: null,
      error_message: null,
    },
    {
      id: "r4",
      series_name: "Casse",
      status: "error",
      anilist_id: null,
      anilist_title: null,
      anilist_url: null,
      error_message: "network error",
    },
  ];

  it("renders nothing when there are no results", () => {
    const { container } = render(
      <ReadingStatusMatchResultsCard results={[]} libraryId="lib-1" t={t} />
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders anilist links, ids and status labels", () => {
    render(<ReadingStatusMatchResultsCard results={results} libraryId="lib-1" t={t} />);

    const links = screen.getAllByRole("link", { name: "Berserk" });
    expect(
      links.some((l) => l.getAttribute("href") === "https://anilist.co/manga/30002")
    ).toBe(true);
    expect(screen.getByText("#30002")).toBeInTheDocument();
    expect(screen.getByText("Sans URL")).toBeInTheDocument();
    expect(screen.getAllByText("jobDetail.linked").length).toBeGreaterThan(0);
    expect(screen.getByText("jobDetail.ambiguous")).toBeInTheDocument();
    expect(screen.getByText("common.error")).toBeInTheDocument();
    expect(screen.getByText("network error")).toBeInTheDocument();
  });
});

describe("ReadingStatusPushReportCard", () => {
  it("renders the series count and every stat", () => {
    const report: ReadingStatusPushReportDto = {
      job_id: "job-1",
      status: "success",
      total_series: 6,
      pushed: 3,
      skipped: 1,
      no_books: 1,
      errors: 1,
    };

    render(<ReadingStatusPushReportCard report={report} t={t} />);

    expect(screen.getByText("jobDetail.readingStatusPushReport")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.seriesAnalyzed:6")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.pushed")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.noBooks")).toBeInTheDocument();
  });
});

describe("ReadingStatusPushResultsCard", () => {
  const results: ReadingStatusPushResultDto[] = [
    {
      id: "r1",
      series_id: "s1",
      series_name: "Berserk",
      status: "pushed",
      anilist_id: 30002,
      anilist_title: "Berserk",
      anilist_url: "https://anilist.co/manga/30002",
      anilist_status: "READING",
      progress_volumes: 12,
      error_message: null,
    },
    {
      id: "r2",
      series_name: "Ignorée",
      status: "skipped",
      anilist_id: null,
      anilist_title: null,
      anilist_url: null,
      anilist_status: null,
      progress_volumes: null,
      error_message: null,
    },
    {
      id: "r3",
      series_name: "Sans tome",
      status: "no_books",
      anilist_id: null,
      anilist_title: null,
      anilist_url: null,
      anilist_status: null,
      progress_volumes: null,
      error_message: null,
    },
    {
      id: "r4",
      series_name: "Casse",
      status: "error",
      anilist_id: null,
      anilist_title: null,
      anilist_url: null,
      anilist_status: null,
      progress_volumes: null,
      error_message: "push failed",
    },
  ];

  it("renders nothing when there are no results", () => {
    const { container } = render(
      <ReadingStatusPushResultsCard results={[]} libraryId="lib-1" t={t} />
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders pushed details and status labels", () => {
    render(<ReadingStatusPushResultsCard results={results} libraryId="lib-1" t={t} />);

    const links = screen.getAllByRole("link", { name: "Berserk" });
    expect(
      links.some((l) => l.getAttribute("href") === "https://anilist.co/manga/30002")
    ).toBe(true);
    expect(screen.getByText("READING")).toBeInTheDocument();
    expect(screen.getByText("vol. 12")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.pushed")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.skipped")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.noBooks")).toBeInTheDocument();
    expect(screen.getByText("push failed")).toBeInTheDocument();
  });
});
