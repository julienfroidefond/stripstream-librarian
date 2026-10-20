import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";
import { revalidateReadingLists } from "@/lib/readingListsCache";

type Params = Promise<{ id: string }>;

export const GET = withRoute(async (_request: NextRequest, { params }: { params: Params }) => {
  const { id } = await params;
  const data = await apiFetch(`/reading-lists/${id}`);
  return NextResponse.json(data);
}, { fallback: "Failed" });

export const PATCH = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { id } = await params;
  const body = await request.json();
  const data = await apiFetch(`/reading-lists/${id}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
  revalidateReadingLists();
  return NextResponse.json(data);
}, { fallback: "Failed" });

export const DELETE = withRoute(async (_request: NextRequest, { params }: { params: Params }) => {
  const { id } = await params;
  await apiFetch(`/reading-lists/${id}`, { method: "DELETE" });
  revalidateReadingLists();
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed" });
