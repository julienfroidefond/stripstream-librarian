import { NextResponse, NextRequest } from "next/server";
import { revalidatePath } from "next/cache";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = Promise<{ seriesId: string }>;

export const PUT = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  const body = await request.json();
  await apiFetch(`/series/${seriesId}/rating`, {
    method: "PUT",
    body: JSON.stringify(body),
  });
  revalidatePath(`/series/${seriesId}`);
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed to save rating" });

export const DELETE = withRoute(async (_request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  await apiFetch(`/series/${seriesId}/rating`, { method: "DELETE" });
  revalidatePath(`/series/${seriesId}`);
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed to delete rating" });
