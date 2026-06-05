import { NextRequest, NextResponse } from "next/server";
import { apiFetch } from "@/lib/api";

type Params = { params: Promise<{ path: string[] }> };

async function proxy(request: NextRequest, { params }: Params) {
  const { path } = await params;
  const apiPath = "/telegram-monitor/" + path.join("/") + request.nextUrl.search;

  try {
    let body: unknown = undefined;
    if (request.method !== "GET" && request.method !== "DELETE") {
      const text = await request.text();
      body = text ? JSON.parse(text) : undefined;
    }

    const data = await apiFetch<unknown>(apiPath, {
      method: request.method,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
    return NextResponse.json(data ?? { ok: true });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Request failed";
    const status = message.includes("(404)") ? 404 : message.includes("(400)") ? 400 : 500;
    return NextResponse.json({ error: message }, { status });
  }
}

export const GET = proxy;
export const POST = proxy;
export const DELETE = proxy;
