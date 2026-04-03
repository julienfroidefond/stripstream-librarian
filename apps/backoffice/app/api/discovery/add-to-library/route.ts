import { NextResponse, NextRequest } from "next/server";
import { revalidatePath } from "next/cache";
import { apiFetch } from "@/lib/api";

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const data = await apiFetch("/discovery/add-to-library", {
      method: "POST",
      body: JSON.stringify(body),
    });
    revalidatePath("/series");
    revalidatePath("/libraries");
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed to add series";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
