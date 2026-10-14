import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(
  async () => {
    const data = await apiFetch("/ddl-providers/search-settings");
    return NextResponse.json(data);
  },
  { fallback: "Failed to load the DDL search settings" }
);

export const POST = withRoute(
  async (request: NextRequest) => {
    const body = await request.json();
    const data = await apiFetch("/ddl-providers/search-settings", {
      method: "POST",
      body: JSON.stringify(body),
    });
    return NextResponse.json(data);
  },
  {
    fallback: "Failed to update the DDL search settings",
    statusFromError: (_error, message) => (message.includes("(400)") ? 400 : 500),
  }
);
