import { NextResponse, NextRequest } from "next/server";
import { revalidatePath } from "next/cache";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = Promise<{ seriesId: string }>;

export const PATCH = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  const body = await request.json();
  const data = await apiFetch(`/series/${seriesId}`, {
    method: "PATCH",
    body: JSON.stringify(body),
  });
  return NextResponse.json(data);
}, { fallback: "Failed to update series" });

export const DELETE = withRoute(async (request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  const data = await apiFetch(`/series/${seriesId}`, { method: "DELETE" });
  revalidatePath("/series");
  return NextResponse.json(data);
}, { fallback: "Failed to delete series" });
