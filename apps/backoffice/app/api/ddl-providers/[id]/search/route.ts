import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

type Params = { params: Promise<{ id: string }> };

export const POST = withRoute(
  async (request: NextRequest, { params }: Params) => {
    const { id } = await params;
    const body = await request.json().catch(() => ({}));
    const data = await apiFetch(`/ddl-providers/${encodeURIComponent(id)}/search`, {
      method: "POST",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  },
  {
    fallback: "Failed to search DDL provider",
    statusFromError: (_error, message) =>
      message.includes("(404)") ? 404 : message.includes("(400)") ? 400 : 500,
  }
);
