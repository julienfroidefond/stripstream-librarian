import { NextResponse } from "next/server";
import { listJobs } from "@/lib/api";

export async function GET() {
  try {
    const data = await listJobs();
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Failed to fetch jobs" }, { status: 500 });
  }
}
