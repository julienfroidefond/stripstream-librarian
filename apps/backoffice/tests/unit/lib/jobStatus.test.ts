import { describe, expect, it } from "vitest";

import {
  TERMINAL_JOB_STATUSES,
  isActiveJobStatus,
  isCancelledJobStatus,
  isCompletedJobStatus,
  isFailedJobStatus,
  isPartialJobStatus,
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
  "partial",
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

describe("TERMINAL_JOB_STATUSES", () => {
  it("is the single source of truth for the four terminal statuses", () => {
    expect([...TERMINAL_JOB_STATUSES]).toEqual([
      "success",
      "partial",
      "failed",
      "cancelled",
    ]);
  });

  it("matches the terminal predicates exactly", () => {
    for (const status of ALL_STATUSES) {
      const expected =
        isCompletedJobStatus(status) ||
        isPartialJobStatus(status) ||
        isFailedJobStatus(status) ||
        isCancelledJobStatus(status);
      expect((TERMINAL_JOB_STATUSES as readonly string[]).includes(status)).toBe(
        expected
      );
    }
  });
});

describe("terminal predicates", () => {
  it("classifies each terminal status exactly once", () => {
    expect(isCompletedJobStatus("success")).toBe(true);
    expect(isPartialJobStatus("partial")).toBe(true);
    expect(isFailedJobStatus("failed")).toBe(true);
    expect(isCancelledJobStatus("cancelled")).toBe(true);
  });

  it("does not confuse partial with the other terminal statuses", () => {
    expect(isCompletedJobStatus("partial")).toBe(false);
    expect(isFailedJobStatus("partial")).toBe(false);
    expect(isCancelledJobStatus("partial")).toBe(false);
  });

  it("isTerminalJobStatus is the union of the four", () => {
    for (const status of ALL_STATUSES) {
      const expected =
        isCompletedJobStatus(status) ||
        isPartialJobStatus(status) ||
        isFailedJobStatus(status) ||
        isCancelledJobStatus(status);
      expect(isTerminalJobStatus(status)).toBe(expected);
    }
  });

  it("never reports a status as both active and terminal", () => {
    for (const status of ALL_STATUSES) {
      expect(isActiveJobStatus(status) && isTerminalJobStatus(status)).toBe(false);
    }
  });
});
