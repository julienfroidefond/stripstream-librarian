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

export function isCancelledJobStatus(status: string): boolean {
  return status === "cancelled";
}

export function isTerminalJobStatus(status: string): boolean {
  return isCompletedJobStatus(status) || isFailedJobStatus(status) || isCancelledJobStatus(status);
}
