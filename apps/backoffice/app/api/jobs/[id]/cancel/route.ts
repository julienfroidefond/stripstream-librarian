import { NextRequest, NextResponse } from "next/server";
import { cancelJob } from "@/lib/api";

export async function POST(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;
  try {
    const data = await cancelJob(id);
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Failed to cancel job" }, { status: 500 });
  }
}
