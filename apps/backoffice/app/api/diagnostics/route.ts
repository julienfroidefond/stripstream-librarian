import { NextResponse } from "next/server";
import { fetchDiagnostics } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const dynamic = "force-dynamic";

/** Full diagnostics payload, used by the "Copy diagnostics" button. */
export const GET = withRoute(async () => {
  const data = await fetchDiagnostics();
  return NextResponse.json(data);
}, { message: "Failed to fetch diagnostics" });
