import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

type Params = Promise<{ id: string }>;

export async function GET(_request: NextRequest, { params }: { params: Params }) {
  try {
    const { id } = await params;
    const data = await apiFetch(`/reading-lists/${id}`);
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function PATCH(request: NextRequest, { params }: { params: Params }) {
  try {
    const { id } = await params;
    const body = await request.json();
    const data = await apiFetch(`/reading-lists/${id}`, {
      method: "PATCH",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

export async function DELETE(_request: NextRequest, { params }: { params: Params }) {
  try {
    const { id } = await params;
    await apiFetch(`/reading-lists/${id}`, { method: "DELETE" });
    return new NextResponse(null, { status: 204 });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Failed";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
