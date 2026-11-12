/** Browser-side diagnostics calls. */

import type { DiagnosticsProbe, DiagnosticsResponse } from "../diagnostics";
import { browserFetch } from "./transport";

export const diagnosticsApi = {
  get(): Promise<DiagnosticsResponse> {
    return browserFetch<DiagnosticsResponse>("/api/diagnostics");
  },

  probe(): Promise<DiagnosticsProbe> {
    return browserFetch<DiagnosticsProbe>("/api/diagnostics/probe");
  },
};
