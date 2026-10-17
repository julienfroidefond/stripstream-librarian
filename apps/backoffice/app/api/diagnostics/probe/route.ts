import { NextResponse } from "next/server";
import { fetchDiagnosticsProbe } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const dynamic = "force-dynamic";

/**
 * On-demand probe: external integrations + missing book files.
 *
 * Deliberately a separate route from `/api/diagnostics`: it contacts
 * third-party services and stats every book file, so it must never run on a
 * page load.
 */
export const GET = withRoute(async () => {
  const data = await fetchDiagnosticsProbe();
  return NextResponse.json(data);
}, { message: "Failed to run diagnostics probe" });
