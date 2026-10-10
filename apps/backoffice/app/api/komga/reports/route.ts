import { NextResponse } from "next/server";
import { listKomgaReports } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await listKomgaReports();
  return NextResponse.json(data);
}, { fallback: "Failed to fetch reports" });
