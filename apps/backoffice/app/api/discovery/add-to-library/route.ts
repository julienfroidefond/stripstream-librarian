import { NextResponse, NextRequest } from "next/server";
import { revalidatePath } from "next/cache";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch("/discovery/add-to-library", {
    method: "POST",
    body: JSON.stringify(body),
  });
  revalidatePath("/series");
  revalidatePath("/libraries");
  return NextResponse.json(data);
}, { fallback: "Failed to add series" });
