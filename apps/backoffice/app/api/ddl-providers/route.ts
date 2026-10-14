import { NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";
import { withRoute } from "@/lib/api-handler";

export const GET = withRoute(async () => {
  const data = await apiFetch("/ddl-providers");
  return NextResponse.json(data);
}, { fallback: "Failed to list DDL providers" });
