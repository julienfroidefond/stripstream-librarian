import { NextResponse, NextRequest } from "next/server";
import { revalidatePath, revalidateTag } from "next/cache";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = Promise<{ seriesId: string }>;

function revalidateShelf(seriesId: string): void {
  revalidatePath(`/series/${seriesId}`);
  revalidatePath("/series");
  // Series lists/details are cached under these tags in admin mode.
  revalidateTag("series", { expire: 0 });
  revalidateTag(`series:${seriesId}`, { expire: 0 });
}

export const PUT = withRoute(async (_request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  await apiFetch(`/series/${seriesId}/shelf`, { method: "PUT" });
  revalidateShelf(seriesId);
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed to set series aside" });

export const DELETE = withRoute(async (_request: NextRequest, { params }: { params: Params }) => {
  const { seriesId } = await params;
  await apiFetch(`/series/${seriesId}/shelf`, { method: "DELETE" });
  revalidateShelf(seriesId);
  return new NextResponse(null, { status: 204 });
}, { fallback: "Failed to resume series" });
