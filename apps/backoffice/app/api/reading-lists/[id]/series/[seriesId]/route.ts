import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { revalidateReadingLists } from "@/lib/readingListsCache";

type Params = Promise<{ id: string; seriesId: string }>;

export const DELETE = withRoute(async (_request: NextRequest, { params }: { params: Params }) => {
  const { id, seriesId } = await params;
  await apiFetch(`/reading-lists/${id}/series/${seriesId}`, { method: "DELETE" });
  revalidateReadingLists();
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed" });

export const POST = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { id, seriesId } = await params;
  const body = (await request.json()) as { to_list_id?: string };
  await apiFetch(`/reading-lists/${id}/series/${seriesId}/move`, {
    method: "POST",
    body: JSON.stringify({ to_list_id: body.to_list_id }),
  });
  revalidateReadingLists();
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed" });
