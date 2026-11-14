// @vitest-environment node
import { describe, expect, it } from "vitest";

import {
  SESSION_SECRET_MIN_LENGTH,
  readSessionSecret,
  requireSessionSecret,
} from "@/lib/auth/secret";

const VALID = "x".repeat(SESSION_SECRET_MIN_LENGTH);

describe("readSessionSecret", () => {
  it("encodes a secret of at least 32 characters", () => {
    expect(readSessionSecret(VALID)).toEqual(new TextEncoder().encode(VALID));
  });

  it.each([undefined, "", "x".repeat(SESSION_SECRET_MIN_LENGTH - 1)])(
    "returns null for %j",
    (value) => {
      expect(readSessionSecret(value)).toBeNull();
    },
  );
});

describe("requireSessionSecret", () => {
  it("throws when the secret is too short", () => {
    expect(() => requireSessionSecret("short")).toThrow("SESSION_SECRET");
  });

  it("returns the encoded secret otherwise", () => {
    expect(requireSessionSecret(VALID)).toBeInstanceOf(Uint8Array);
  });
});
