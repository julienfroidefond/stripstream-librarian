import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(async (request: NextRequest) => {
  const body = await request.json();
  const data = await apiFetch("/reading-lists/ai-suggest", {
    method: "POST",
    body: JSON.stringify(body),
  });
  return NextResponse.json(data);
}, {
  fallback: "Failed to suggest reading lists",
  statusFromError: (_error, message) => (message.includes("(422)") ? 422 : 500),
});
