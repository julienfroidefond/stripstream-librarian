import { NextRequest, NextResponse } from "next/server";
import { listFolders } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async (request: NextRequest) => {
  const { searchParams } = new URL(request.url);
  const path = searchParams.get("path") || undefined;
  const data = await listFolders(path);
  return NextResponse.json(data);
}, { message: "Failed to fetch folders" });
