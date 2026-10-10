import { NextRequest, NextResponse } from "next/server";
import { cancelJob } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> }) => {
  const { id } = await params;
  const data = await cancelJob(id);
  return NextResponse.json(data);
}, { message: "Failed to cancel job" });
