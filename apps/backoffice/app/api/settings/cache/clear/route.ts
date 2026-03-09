import { NextResponse } from "next/server";
import { clearCache } from "@/lib/api";

export async function POST() {
  try {
    const data = await clearCache();
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Failed to clear cache" }, { status: 500 });
  }
}
