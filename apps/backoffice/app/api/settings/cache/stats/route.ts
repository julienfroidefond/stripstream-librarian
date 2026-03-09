import { NextResponse } from "next/server";
import { getCacheStats } from "@/lib/api";

export async function GET() {
  try {
    const data = await getCacheStats();
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Failed to fetch cache stats" }, { status: 500 });
  }
}
