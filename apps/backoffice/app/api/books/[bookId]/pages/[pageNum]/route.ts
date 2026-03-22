import { NextRequest, NextResponse } from "next/server";
import { config } from "@/lib/api";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string; pageNum: string }> }
) {
  const { bookId, pageNum } = await params;
  try {
    const { baseUrl, token } = config();
    const { searchParams } = new URL(request.url);
    const format = searchParams.get("format") || "webp";
    const width = searchParams.get("width") || "";
    const quality = searchParams.get("quality") || "";

    const apiUrl = new URL(`${baseUrl}/books/${bookId}/pages/${pageNum}`);
    apiUrl.searchParams.set("format", format);
    if (width) apiUrl.searchParams.set("width", width);
    if (quality) apiUrl.searchParams.set("quality", quality);

    const response = await fetch(apiUrl.toString(), {
      headers: { Authorization: `Bearer ${token}` },
    });

    if (!response.ok) {
      return new NextResponse(`Failed to fetch image: ${response.status}`, {
        status: response.status
      });
    }

    const contentType = response.headers.get("content-type") || "image/webp";

    return new NextResponse(response.body, {
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
