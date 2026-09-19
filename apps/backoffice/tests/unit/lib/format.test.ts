import { describe, expect, it } from "vitest";

import {
  formatBytes,
  formatDateTime,
  formatDuration,
  formatEta,
  formatNumber,
  formatRate,
  formatSpeed,
  formatVolumeRange,
  formatVolumes,
} from "@/lib/format";

describe("formatBytes", () => {
  it("formats zero with the smallest unit", () => {
    expect(formatBytes(0)).toBe("0 B");
  });

  it("keeps single bytes with one decimal", () => {
    expect(formatBytes(512)).toBe("512.0 B");
  });

  it("scales through the binary units", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.0 GB");
    expect(formatBytes(1024 * 1024 * 1024 * 1024)).toBe("1.0 TB");
  });

  it("rounds to a single decimal", () => {
    expect(formatBytes(1536)).toBe("1.5 KB");
  });
});

describe("formatNumber", () => {
  it("groups thousands with the locale separator", () => {
    expect(formatNumber(1234, "en")).toBe("1,234");
  });

  it("falls back to en-US for unknown locales", () => {
    expect(formatNumber(1234, "de")).toBe("1,234");
  });
});

describe("formatDuration", () => {
  const start = "2024-01-01T00:00:00.000Z";

  it("returns seconds under a minute", () => {
    expect(formatDuration(start, "2024-01-01T00:00:45.000Z")).toBe("45s");
  });

  it("returns minutes and seconds under an hour", () => {
    expect(formatDuration(start, "2024-01-01T00:01:30.000Z")).toBe("1m 30s");
  });

  it("returns hours and minutes beyond an hour", () => {
    expect(formatDuration(start, "2024-01-01T02:05:00.000Z")).toBe("2h 5m");
  });
});

describe("formatRate", () => {
  it("returns a dash when there is nothing to rate", () => {
    expect(formatRate(0, 1000)).toBe("-");
    expect(formatRate(10, 0)).toBe("-");
  });

  it("computes items per second", () => {
    expect(formatRate(10, 1000)).toBe("10.0/s");
    expect(formatRate(5, 500)).toBe("10.0/s");
  });
});

describe("formatSpeed", () => {
  it("uses B/s below a kilobyte", () => {
    expect(formatSpeed(512)).toBe("512 B/s");
  });

  it("uses KB/s below a megabyte", () => {
    expect(formatSpeed(2048)).toBe("2.0 KB/s");
  });

  it("uses MB/s above a megabyte", () => {
    expect(formatSpeed(1024 * 1024 * 2)).toBe("2.0 MB/s");
  });
});

describe("formatEta", () => {
  it("returns an empty string for invalid values", () => {
    expect(formatEta(0)).toBe("");
    expect(formatEta(-5)).toBe("");
    expect(formatEta(8640000)).toBe("");
  });

  it("formats seconds, minutes and hours", () => {
    expect(formatEta(30)).toBe("30s");
    expect(formatEta(90)).toBe("1m30s");
    expect(formatEta(3661)).toBe("1h01m");
  });
});

describe("formatVolumes", () => {
  it("sorts and zero-pads the volume numbers", () => {
    expect(formatVolumes([3, 1, 2])).toBe("T01, T02, T03");
    expect(formatVolumes([10, 2])).toBe("T02, T10");
  });

  it("handles an empty list", () => {
    expect(formatVolumes([])).toBe("");
  });
});

describe("formatVolumeRange", () => {
  it("returns a dash for an empty list", () => {
    expect(formatVolumeRange([])).toBe("—");
  });

  it("lists every value when there are three or fewer", () => {
    expect(formatVolumeRange([1, 2, 3])).toBe("1, 2, 3");
  });

  it("compresses longer ranges with a count", () => {
    expect(formatVolumeRange([1, 2, 3, 4, 5])).toBe("1-5 (5)");
  });
});

describe("formatDateTime", () => {
  it("returns the raw value when the date is invalid", () => {
    expect(formatDateTime("not-a-date")).toBe("not-a-date");
  });

  it("formats a valid ISO date", () => {
    const formatted = formatDateTime("2024-01-01T12:00:00.000Z");
    expect(formatted).not.toBe("2024-01-01T12:00:00.000Z");

    expect(formatted).toMatch(/2024/);
  });
});
