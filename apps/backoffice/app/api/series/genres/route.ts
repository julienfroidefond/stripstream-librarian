import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

export async function GET() {
  try {
    const data = await apiFetch<string[]>("/series/genres");
    return NextResponse.json(data);
  } catch {
    return NextResponse.json([], { status: 200 });
  }
}
