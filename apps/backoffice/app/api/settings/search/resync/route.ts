import { NextResponse } from "next/server";
import { forceSearchResync } from "@/lib/api";

export async function POST() {
  try {
    const data = await forceSearchResync();
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Failed to trigger search resync" }, { status: 500 });
  }
}
