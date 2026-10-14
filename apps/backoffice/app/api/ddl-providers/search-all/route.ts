import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const POST = withRoute(
  async () => {
    const data = await apiFetch("/ddl-providers/search-all", { method: "POST" });
    return NextResponse.json(data);
  },
  {
    fallback: "Failed to start the DDL background search",
    statusFromError: (_error, message) => (message.includes("(400)") ? 400 : 500),
  }
);
