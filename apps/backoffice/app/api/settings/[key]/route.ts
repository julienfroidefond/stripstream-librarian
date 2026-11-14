import { NextRequest, NextResponse } from "next/server";
import { apiFetch, updateSetting } from "@/lib/api";
import { ApiError } from "@/lib/api/errors";
import { invalidateAfter } from "@/lib/cache";
import { errorResponse, withRoute } from "@/lib/server/api-handler";

export const GET = withRoute(async (
  _request: NextRequest,
  { params }: { params: Promise<{ key: string }> }) => {
  const { key } = await params;
  const data = await apiFetch<unknown>(`/settings/${key}`);
  return NextResponse.json(data);
}, { message: "Failed to fetch setting" });

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ key: string }> },
) {
  try {
    const { key } = await params;
    const { value } = await request.json();
    const data = await updateSetting(key, value);
    invalidateAfter({ type: "settings.changed", key });
    return NextResponse.json(data);
  } catch (error) {
    // Forward an upstream validation error (status + detail) so the draft can
    // surface it instead of the generic "Failed to update setting".
    if (error instanceof ApiError) {
      return NextResponse.json(
        { error: error.detail ?? error.message },
        { status: error.status },
      );
    }
    return errorResponse(error, { message: "Failed to update setting" });
  }
}
