import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = { params: Promise<{ id: string }> };

export const DELETE = withRoute(
  async (request: NextRequest, { params }: Params) => {
    const { id } = await params;
    const hard = request.nextUrl.searchParams.get("hard");
    const query = hard ? `?hard=${encodeURIComponent(hard)}` : "";
    await apiFetch(`/ddl-provider-downloads/${encodeURIComponent(id)}${query}`, {
      method: "DELETE",
    });
    return new NextResponse(null, { status: 204 });
  },
  {
    fallback: "Failed to dismiss download",
    statusFromError: (_error, message) => (message.includes("(404)") ? 404 : 500),
  }
);
