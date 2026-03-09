import { NextRequest, NextResponse } from "next/server";

export async function POST(request: NextRequest) {
  try {
    const baseUrl = process.env.API_BASE_URL || "http://api:7080";
    const token = process.env.API_BOOTSTRAP_TOKEN;
    
    const response = await fetch(`${baseUrl}/settings/cache/clear`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
      },
      cache: "no-store"
    });

    if (!response.ok) {
      return NextResponse.json({ error: "Failed to clear cache" }, { status: response.status });
    }

    const data = await response.json();
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Internal server error" }, { status: 500 });
  }
}
