import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

type Params = Promise<{ name: string }>;

export async function PATCH(request: NextRequest, { params }: { params: Params }) {
  try {
    const { name } = await params;
    const body = await request.json();
    const data = await apiFetch(`/genres/${encodeURIComponent(name)}`, {
      method: "PATCH",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function DELETE(request: NextRequest, { params }: { params: Params }) {
  try {
    const { name } = await params;
    const data = await apiFetch(`/genres/${encodeURIComponent(name)}`, {
      method: "DELETE",
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
