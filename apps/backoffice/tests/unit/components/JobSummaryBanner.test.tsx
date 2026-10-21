import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { JobSummaryBanner } from "@/app/(app)/jobs/[id]/components/JobSummaryBanner";
import type { MetadataBatchReportDto } from "@/lib/api";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

const t = ((key: string) => key) as unknown as TranslateFunction;

function renderBanner(report: MetadataBatchReportDto) {
  return render(
    <JobSummaryBanner
      job={{
        type: "metadata_batch_rematch",
        status: "success",
        started_at: "2024-01-01T10:00:00Z",
        finished_at: "2024-01-01T10:05:00Z",
        error_opt: null,
        stats_json: null,
        total_files: 12,
        processed_files: 12,
      }}
      batchReport={report}
      refreshReport={null}
      readingStatusReport={null}
      readingStatusPushReport={null}
      downloadDetectionReport={null}
      t={t}
      formatDuration={() => "5min"}
    />
  );
}

describe("JobSummaryBanner", () => {
  it("shows every metadata batch outcome, including low confidence and errors", () => {
    const { container } = renderBanner({
      job_id: "job-1",
      status: "success",
      total_series: 12,
      processed: 12,
      auto_matched: 5,
      no_results: 2,
      too_many_results: 1,
      low_confidence: 3,
      already_linked: 4,
      errors: 6,
    });

    const text = container.textContent ?? "";
    expect(text).toContain("5 jobdetail.automatched");
    expect(text).toContain("4 jobdetail.alreadylinked");
    expect(text).toContain("2 jobdetail.noresults");
    expect(text).toContain("1 jobdetail.toomanyresults");
    expect(text).toContain("3 jobdetail.lowconfidence");
    expect(text).toContain("6 jobdetail.errors");
  });

  it("renders no metadata counters when the report is unavailable", () => {
    const { container } = render(
      <JobSummaryBanner
        job={{
          type: "metadata_batch_rematch",
          status: "success",
          started_at: "2024-01-01T10:00:00Z",
          finished_at: "2024-01-01T10:05:00Z",
          error_opt: null,
          stats_json: null,
          total_files: 12,
          processed_files: 12,
        }}
        batchReport={null}
        refreshReport={null}
        readingStatusReport={null}
        readingStatusPushReport={null}
        downloadDetectionReport={null}
        t={t}
        formatDuration={() => "5min"}
      />
    );

    expect(container.textContent).not.toContain("jobdetail.automatched");
  });
});
