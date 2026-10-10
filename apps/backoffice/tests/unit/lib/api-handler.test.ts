// @vitest-environment node
import { NextResponse } from "next/server";
import { describe, expect, it, vi } from "vitest";

import { errorResponse, withRoute } from "@/lib/api-handler";

describe("errorResponse", () => {
  it("uses the thrown Error message", async () => {
    const res = errorResponse(new Error("boom"));
    expect(res.status).toBe(500);
    expect(await res.json()).toEqual({ error: "boom" });
  });

  it("falls back to a default message for non-Error values", async () => {
    const res = errorResponse("nope", { fallback: "Failed to do the thing" });
    expect(await res.json()).toEqual({ error: "Failed to do the thing" });
  });

  it("prefers a constant message when provided", async () => {
    const res = errorResponse(new Error("internal detail"), { message: "Failed to clear cache" });
    expect(await res.json()).toEqual({ error: "Failed to clear cache" });
  });

  it("applies a custom default status", () => {
    expect(errorResponse(new Error("missing"), { status: 404 }).status).toBe(404);
  });

  it("lets statusFromError override the default status", async () => {
    const res = errorResponse(new Error("API ... failed (409): conflict"), {
      statusFromError: (_error, message) => {
        const match = message.match(/failed \((\d+)\)/);
        return match ? parseInt(match[1], 10) : 500;
      },
    });
    expect(res.status).toBe(409);
  });

  it("runs the onError side effect with the resolved message", () => {
    const onError = vi.fn();
    errorResponse(new Error("boom"), { onError });
    expect(onError).toHaveBeenCalledWith(expect.any(Error), "boom");
  });
});

describe("withRoute", () => {
  it("returns the handler response on success", async () => {
    const handler = withRoute(async () => NextResponse.json({ ok: true }));
    const res = await handler();
    expect(res.status).toBe(200);
    expect(await res.json()).toEqual({ ok: true });
  });

  it("forwards the request and context to the handler", async () => {
    const request = { url: "http://localhost/things/1" } as never;
    const context = { params: Promise.resolve({ id: "1" }) };
    const seen = vi.fn();
    const handler = withRoute(async (req, ctx: typeof context) => {
      seen(req, await ctx.params);
      return NextResponse.json({});
    });
    await handler(request, context);
    expect(seen).toHaveBeenCalledWith(request, { id: "1" });
  });

  it("returns a JSON error response when the handler throws", async () => {
    const handler = withRoute(
      async () => {
        throw new Error("boom");
      },
      { fallback: "Failed" }
    );
    const res = await handler();
    expect(res.status).toBe(500);
    expect(await res.json()).toEqual({ error: "boom" });
  });

  it("uses the fallback when the handler throws a non-Error", async () => {
    const handler = withRoute(
      async () => {
        throw "oops";
      },
      { fallback: "Failed" }
    );
    expect(await (await handler()).json()).toEqual({ error: "Failed" });
  });
});
