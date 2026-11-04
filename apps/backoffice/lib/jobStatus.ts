/// Statuses that end a job's lifecycle (no further progress expected).
/// Kept in one place so the SSE poller and the UI agree on when to stop.
export const TERMINAL_JOB_STATUSES = [
  "success",
  "partial",
  "failed",
  "cancelled",
] as const;

export type TerminalJobStatus = (typeof TERMINAL_JOB_STATUSES)[number];

export function isPhase2Status(status: string): boolean {
  return status === "extracting_pages" || status === "generating_thumbnails";
}

export function isRunningJobStatus(status: string): boolean {
  return status === "running" || isPhase2Status(status);
}

export function isActiveJobStatus(status: string): boolean {
  return status === "running" || status === "pending" || isPhase2Status(status);
}

export function isCompletedJobStatus(status: string): boolean {
  return status === "success";
}

export function isFailedJobStatus(status: string): boolean {
  return status === "failed";
}

export function isPartialJobStatus(status: string): boolean {
  return status === "partial";
}

export function isCancelledJobStatus(status: string): boolean {
  return status === "cancelled";
}

export function isTerminalJobStatus(status: string): boolean {
  return (TERMINAL_JOB_STATUSES as readonly string[]).includes(status);
}
