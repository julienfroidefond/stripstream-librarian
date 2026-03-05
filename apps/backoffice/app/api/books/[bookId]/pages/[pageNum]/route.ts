import { NextRequest, NextResponse } from "next/server";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string; pageNum: string }> }
) {
  const { bookId, pageNum } = await params;
  
  // Récupérer les query params (format, width, quality)
  const { searchParams } = new URL(request.url);
  const format = searchParams.get("format") || "webp";
  const width = searchParams.get("width") || "";
  const quality = searchParams.get("quality") || "";
  
  // Construire l'URL vers l'API backend
  const apiBaseUrl = process.env.API_BASE_URL || "http://api:8080";
  const apiUrl = new URL(`${apiBaseUrl}/books/${bookId}/pages/${pageNum}`);
  apiUrl.searchParams.set("format", format);
  if (width) apiUrl.searchParams.set("width", width);
  if (quality) apiUrl.searchParams.set("quality", quality);
  
  // Faire la requête à l'API
  const token = process.env.API_BOOTSTRAP_TOKEN;
  if (!token) {
    return new NextResponse("API token not configured", { status: 500 });
  }
  
  try {
    const response = await fetch(apiUrl.toString(), {
      headers: {
        Authorization: `Bearer ${token}`,
      },
    });
    
    if (!response.ok) {
      return new NextResponse(`Failed to fetch image: ${response.status}`, { 
        status: response.status 
      });
    }
    
    // Récupérer le content-type et les données
    const contentType = response.headers.get("content-type") || "image/webp";
    const imageBuffer = await response.arrayBuffer();
    
    // Retourner l'image avec le bon content-type
    return new NextResponse(imageBuffer, {
      headers: {
        "Content-Type": contentType,
        "Cache-Control": "public, max-age=300",
      },
    });
  } catch (error) {
    console.error("Error fetching image:", error);
    return new NextResponse("Failed to fetch image", { status: 500 });
  }
}
