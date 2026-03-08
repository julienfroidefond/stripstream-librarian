import { NextRequest, NextResponse } from "next/server";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }
) {
  const { bookId } = await params;
  
  const apiBaseUrl = process.env.API_BASE_URL || "http://api:8080";
  const apiUrl = `${apiBaseUrl}/books/${bookId}/thumbnail`;
  
  const token = process.env.API_BOOTSTRAP_TOKEN;
  if (!token) {
    return new NextResponse("API token not configured", { status: 500 });
  }
  
  try {
    const response = await fetch(apiUrl, {
      headers: {
        Authorization: `Bearer ${token}`,
      },
    });
    
    if (!response.ok) {
      return new NextResponse(`Failed to fetch thumbnail: ${response.status}`, { 
        status: response.status 
      });
    }
    
    const contentType = response.headers.get("content-type") || "image/webp";
    const imageBuffer = await response.arrayBuffer();
    
    return new NextResponse(imageBuffer, {
      headers: {
        "Content-Type": contentType,
        "Cache-Control": "public, max-age=31536000, immutable",
      },
    });
  } catch (error) {
    console.error("Error fetching thumbnail:", error);
    return new NextResponse("Failed to fetch thumbnail", { status: 500 });
  }
}
