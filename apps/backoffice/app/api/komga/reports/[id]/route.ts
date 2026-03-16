import { NextResponse, NextRequest } from "next/server";
import { getKomgaReport } from "@/lib/api";

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;
    const data = await getKomgaReport(id);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch report";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
