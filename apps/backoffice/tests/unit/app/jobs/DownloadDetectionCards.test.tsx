import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  DownloadDetectionErrorsCard,
  RssSnapshotCard,
} from "@/app/(app)/jobs/[id]/components/DownloadDetectionCards";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

const t = ((key: string) => key) as TranslateFunction;

describe("RssSnapshotCard", () => {
  it("uses translated column headers", () => {
    render(<RssSnapshotCard t={t} releases={[{ title: "Astérix 01", size: 1024, seeders: 3 }]} />);

    expect(screen.getByText("jobDetail.rssColumnTitle")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.rssColumnIndexer")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.rssColumnCategories")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.rssColumnSize")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.rssColumnSeeders")).toBeInTheDocument();
    expect(screen.getByText("jobDetail.rssColumnDate")).toBeInTheDocument();
    // No hardcoded French left in the header row.
    expect(screen.queryByText("Catégories")).not.toBeInTheDocument();
  });
});

describe("DownloadDetectionErrorsCard", () => {
  it("falls back to the translated unknown-error label", () => {
    render(
      <DownloadDetectionErrorsCard
        t={t}
        results={[
          {
            id: "r1",
            series_name: "Astérix",
            status: "error",
            missing_count: 0,
            available_releases: null,
            error_message: null,
          },
        ]}
      />
    );

    expect(screen.getByText("common.unknownError")).toBeInTheDocument();
    expect(screen.queryByText("Erreur inconnue")).not.toBeInTheDocument();
  });

  it("renders nothing without results", () => {
    const { container } = render(<DownloadDetectionErrorsCard t={t} results={[]} />);

    expect(container).toBeEmptyDOMElement();
  });
});
