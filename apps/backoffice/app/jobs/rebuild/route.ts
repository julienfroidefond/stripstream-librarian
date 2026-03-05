import { revalidatePath } from "next/cache";
import { NextResponse } from "next/server";
import { apiFetch } from "../../../lib/api";

export async function POST(req: Request) {
  const form = await req.formData();
  const libraryId = String(form.get("library_id") || "").trim();
  const body = libraryId ? { library_id: libraryId } : {};

  await apiFetch("/index/rebuild", {
    method: "POST",
    body: JSON.stringify(body)
  }).catch(() => null);

  revalidatePath("/jobs");
  return NextResponse.redirect(new URL("/jobs", req.url));
}
