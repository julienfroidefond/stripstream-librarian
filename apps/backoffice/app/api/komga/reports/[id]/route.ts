import { NextResponse, NextRequest } from "next/server";
import { getKomgaReport } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },) => {
  const { id } = await params;
  const data = await getKomgaReport(id);
  return NextResponse.json(data);
}, { fallback: "Failed to fetch report" });
