import { NextRequest, NextResponse } from "next/server";
import { apiFetch, updateSetting } from "@/lib/api";

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ key: string }> }
) {
  const { key } = await params;
  try {
    const data = await apiFetch<unknown>(`/settings/${key}`);
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Failed to fetch setting" }, { status: 500 });
  }
}

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ key: string }> }
) {
  const { key } = await params;
  try {
    const { value } = await request.json();
    const data = await updateSetting(key, value);
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Failed to update setting" }, { status: 500 });
  }
}
