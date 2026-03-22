import { NextRequest, NextResponse } from "next/server";
import { config } from "@/lib/api";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }
) {
  const { bookId } = await params;

  try {
    const { baseUrl, token } = config();
    const response = await fetch(`${baseUrl}/books/${bookId}/thumbnail`, {
      headers: { Authorization: `Bearer ${token}` },
    });

    if (!response.ok) {
      return new NextResponse(`Failed to fetch thumbnail: ${response.status}`, {
        status: response.status
      });
    }

    const contentType = response.headers.get("content-type") || "image/webp";

    return new NextResponse(response.body, {
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
