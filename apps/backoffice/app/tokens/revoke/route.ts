import { revalidatePath } from "next/cache";
import { NextResponse } from "next/server";
import { apiFetch } from "../../../lib/api";

export async function POST(req: Request) {
  const form = await req.formData();
  const id = String(form.get("id") || "").trim();
  if (id) {
    await apiFetch(`/admin/tokens/${id}`, { method: "DELETE" }).catch(() => null);
  }
  revalidatePath("/tokens");
  return NextResponse.redirect(new URL("/tokens", req.url));
}
