// @vitest-environment node
import { describe, expect, it } from "vitest";

import { GET } from "@/app/health/route";

describe("GET /health", () => {
  it("returns ok as plain text", async () => {
    const res = await GET();
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toBe("text/plain; charset=utf-8");
    expect(await res.text()).toBe("ok");
  });
});
