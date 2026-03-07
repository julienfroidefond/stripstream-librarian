import { NextRequest, NextResponse } from "next/server";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ key: string }> }
) {
  try {
    const { key } = await params;
    const baseUrl = process.env.API_BASE_URL || "http://api:8080";
    const token = process.env.API_BOOTSTRAP_TOKEN;
    
    const response = await fetch(`${baseUrl}/settings/${key}`, {
      headers: {
        Authorization: `Bearer ${token}`,
      },
      cache: "no-store"
    });

    if (!response.ok) {
      return NextResponse.json({ error: "Failed to fetch setting" }, { status: response.status });
    }

    const data = await response.json();
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Internal server error" }, { status: 500 });
  }
}

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ key: string }> }
) {
  try {
    const { key } = await params;
    const baseUrl = process.env.API_BASE_URL || "http://api:8080";
    const token = process.env.API_BOOTSTRAP_TOKEN;
    const body = await request.json();
    
    const response = await fetch(`${baseUrl}/settings/${key}`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
      cache: "no-store"
    });

    if (!response.ok) {
      return NextResponse.json({ error: "Failed to update setting" }, { status: response.status });
    }

    const data = await response.json();
    return NextResponse.json(data);
  } catch (error) {
    return NextResponse.json({ error: "Internal server error" }, { status: 500 });
  }
}
