import { NextResponse, NextRequest } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = Promise<{ seriesId: string }>;

export const POST = withRoute(
  async (request: NextRequest, { params }: { params: Params }) => {
    const { seriesId } = await params;
    const body = await request.json();
    const data = await apiFetch(`/series/${seriesId}/rename-books`, {
      method: "POST",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  },
  {
    fallback: "Failed to rename books",
    // Try to extract HTTP status from the error message (format: "API ... failed (STATUS): ...")
    statusFromError: (_error, message) => {
      const statusMatch = message.match(/failed \((\d+)\)/);
      return statusMatch ? parseInt(statusMatch[1], 10) : 500;
    },
    onError: (_error, message) => console.error("[rename-books] proxy error:", message),
  }
);
