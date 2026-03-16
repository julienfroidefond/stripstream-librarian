import { NextResponse } from "next/server";
import { listKomgaReports } from "@/lib/api";

export async function GET() {
  try {
    const data = await listKomgaReports();
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to fetch reports";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
