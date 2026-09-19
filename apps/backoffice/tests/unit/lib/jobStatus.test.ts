import { describe, expect, it } from "vitest";

import {
  isActiveJobStatus,
  isCancelledJobStatus,
  isCompletedJobStatus,
  isFailedJobStatus,
  isPhase2Status,
  isRunningJobStatus,
  isTerminalJobStatus,
} from "@/lib/jobStatus";

const ALL_STATUSES = [
  "pending",
  "running",
  "extracting_pages",
  "generating_thumbnails",
  "success",
  "failed",
  "cancelled",
];

describe("isPhase2Status", () => {
  it("only accepts the phase 2 statuses", () => {
    expect(isPhase2Status("extracting_pages")).toBe(true);
    expect(isPhase2Status("generating_thumbnails")).toBe(true);
    expect(isPhase2Status("running")).toBe(false);
    expect(isPhase2Status("pending")).toBe(false);
  });
});

describe("isRunningJobStatus", () => {
  it("includes running and both phase 2 statuses", () => {
    expect(isRunningJobStatus("running")).toBe(true);
    expect(isRunningJobStatus("extracting_pages")).toBe(true);
    expect(isRunningJobStatus("generating_thumbnails")).toBe(true);
  });

  it("excludes pending and terminal statuses", () => {
    expect(isRunningJobStatus("pending")).toBe(false);
    expect(isRunningJobStatus("success")).toBe(false);
    expect(isRunningJobStatus("failed")).toBe(false);
  });
});

describe("isActiveJobStatus", () => {
  it("includes pending, running and phase 2 statuses", () => {
    expect(isActiveJobStatus("pending")).toBe(true);
    expect(isActiveJobStatus("running")).toBe(true);
    expect(isActiveJobStatus("extracting_pages")).toBe(true);
    expect(isActiveJobStatus("generating_thumbnails")).toBe(true);
  });

  it("excludes every terminal status", () => {
    expect(isActiveJobStatus("success")).toBe(false);
    expect(isActiveJobStatus("failed")).toBe(false);
    expect(isActiveJobStatus("cancelled")).toBe(false);
  });
});

describe("terminal predicates", () => {
  it("classifies each terminal status exactly once", () => {
    expect(isCompletedJobStatus("success")).toBe(true);
    expect(isFailedJobStatus("failed")).toBe(true);
    expect(isCancelledJobStatus("cancelled")).toBe(true);
  });

  it("isTerminalJobStatus is the union of the three", () => {
    for (const status of ALL_STATUSES) {
      const expected =
        isCompletedJobStatus(status) || isFailedJobStatus(status) || isCancelledJobStatus(status);
      expect(isTerminalJobStatus(status)).toBe(expected);
    }
  });

  it("never reports a status as both active and terminal", () => {
    for (const status of ALL_STATUSES) {
      expect(isActiveJobStatus(status) && isTerminalJobStatus(status)).toBe(false);
    }
  });
});
