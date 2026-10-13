import { NextRequest, NextResponse } from "next/server";
import { config } from "@/lib/api";

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ bookId: string }> }
) {
  const { bookId } = await params;
  try {
    const { baseUrl, token } = config();

    const response = await fetch(`${baseUrl}/books/${bookId}/file`, {
      headers: { Authorization: `Bearer ${token}` },
      cache: "no-store",
    });

    if (!response.ok) {
      return new NextResponse(`Failed to download file: ${response.status}`, {
        status: response.status,
      });
    }

    // Forward the download headers the browser needs and stream the body untouched.
    // Content-Length is deliberately not forwarded: Safari is strict about it when the
    // body goes through the Next.js proxy, and chunked transfer works everywhere.
    const headers: Record<string, string> = {
      "Content-Type": response.headers.get("content-type") || "application/octet-stream",
      "Cache-Control": "private, no-store",
    };
    const disposition = response.headers.get("content-disposition");
    if (disposition) headers["Content-Disposition"] = disposition;

    return new NextResponse(response.body, { headers });
  } catch (error) {
    console.error("Error downloading book file:", error);
    return new NextResponse("Failed to download file", { status: 500 });
  }
}
