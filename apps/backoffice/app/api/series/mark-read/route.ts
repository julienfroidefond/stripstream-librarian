import { NextRequest, NextResponse } from "next/server";
import { revalidatePath } from "next/cache";
import { markSeriesRead } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await markSeriesRead(body.series, body.status ?? "read");
  revalidatePath("/series");
  if (body.series && body.series !== "unclassified") {
    revalidatePath(`/series/${body.series}`);
  }
  return NextResponse.json(data);
}, { fallback: "Failed to mark series" });
