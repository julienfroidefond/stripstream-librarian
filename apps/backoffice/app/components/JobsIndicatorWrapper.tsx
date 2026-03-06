"use client";

import { JobsIndicator } from "./JobsIndicator";

interface JobsIndicatorWrapperProps {
  apiBaseUrl: string;
  apiToken: string;
}

export function JobsIndicatorWrapper({ apiBaseUrl, apiToken }: JobsIndicatorWrapperProps) {
  return <JobsIndicator apiBaseUrl={apiBaseUrl} apiToken={apiToken} />;
}
