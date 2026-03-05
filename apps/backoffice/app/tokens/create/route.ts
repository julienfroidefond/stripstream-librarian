import { revalidatePath } from "next/cache";
import { NextResponse } from "next/server";
import { apiFetch } from "../../../lib/api";

type CreatedToken = { token: string };

export async function POST(req: Request) {
  const form = await req.formData();
  const name = String(form.get("name") || "").trim();
  const scope = String(form.get("scope") || "read").trim();

  let created = "";
  if (name) {
    const res = await apiFetch<CreatedToken>("/admin/tokens", {
      method: "POST",
      body: JSON.stringify({ name, scope })
    }).catch(() => null);
    created = res?.token || "";
  }

  revalidatePath("/tokens");
  const url = new URL("/tokens", req.url);
  if (created) {
    url.searchParams.set("created", created);
  }
  return NextResponse.redirect(url);
}
