import { NextRequest, NextResponse } from "next/server";
import { config } from "@/lib/api";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }
) {
  const { bookId } = await params;

  try {
    const { baseUrl, token } = config();
    const ifNoneMatch = request.headers.get("if-none-match");

    const fetchHeaders: Record<string, string> = {
      Authorization: `Bearer ${token}`,
    };
    if (ifNoneMatch) {
      fetchHeaders["If-None-Match"] = ifNoneMatch;
    }

    const response = await fetch(`${baseUrl}/books/${bookId}/thumbnail`, {
      headers: fetchHeaders,
      next: { revalidate: 86400 },
    });

    // Forward 304 Not Modified as-is
    if (response.status === 304) {
      return new NextResponse(null, { status: 304 });
    }

    if (!response.ok) {
      return new NextResponse(`Failed to fetch thumbnail: ${response.status}`, {
        status: response.status
      });
    }

    const contentType = response.headers.get("content-type") || "image/webp";
    const etag = response.headers.get("etag");

    const headers: Record<string, string> = {
      "Content-Type": contentType,
      "Cache-Control": "public, max-age=31536000, immutable",
    };
    if (etag) {
      headers["ETag"] = etag;
    }

    return new NextResponse(response.body, { headers });
  } catch (error) {
    console.error("Error fetching thumbnail:", error);
    return new NextResponse("Failed to fetch thumbnail", { status: 500 });
  }
}
