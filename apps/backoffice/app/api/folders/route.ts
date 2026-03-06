import { NextRequest, NextResponse } from "next/server";

export async function GET(request: NextRequest) {
  const apiBaseUrl = process.env.API_BASE_URL || "http://api:8080";
  const apiToken = process.env.API_BOOTSTRAP_TOKEN;
  
  if (!apiToken) {
    return NextResponse.json({ error: "API token not configured" }, { status: 500 });
  }
  
  try {
    const { searchParams } = new URL(request.url);
    const path = searchParams.get("path");
    
    let apiUrl = `${apiBaseUrl}/folders`;
    if (path) {
      apiUrl += `?path=${encodeURIComponent(path)}`;
    }
    
    const response = await fetch(apiUrl, {
      headers: {
        Authorization: `Bearer ${apiToken}`,
      },
    });
    
    if (!response.ok) {
      return NextResponse.json(
        { error: `API error: ${response.status}` },
        { status: response.status }
      );
    }
    
    const data = await response.json();
    return NextResponse.json(data);
  } catch (error) {
    console.error("Proxy error:", error);
    return NextResponse.json({ error: "Failed to fetch folders" }, { status: 500 });
  }
}
