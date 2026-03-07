import { NextRequest, NextResponse } from "next/server";

export async function GET(request: NextRequest) {
  try {
    const baseUrl = process.env.API_BASE_URL || "http://api:8080";
    const token = process.env.API_BOOTSTRAP_TOKEN;
    
    const response = await fetch(`${baseUrl}/settings/cache/stats`, {
      headers: {
        Authorization: `Bearer ${token}`,
      },
      cache: "no-store"
    });

    if (!response.ok) {
      return NextResponse.json({ error: "Failed to fetch cache stats" }, { status: response.status });
    }

    const data = await response.json();
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Internal server error" }, { status: 500 });
  }
}
