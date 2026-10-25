// @vitest-environment node
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

import { describe, expect, it, vi } from "vitest";

vi.mock("next/headers", () => ({
  cookies: vi.fn(async () => ({ get: vi.fn() })),
}));

import { metadata } from "@/app/layout";
import {
  APPLE_WEB_APP_META,
  buildSplashLinks,
  SPLASH_LINKS,
  SPLASH_SCREENS,
  splashAssetPath,
  splashMedia,
  type SplashOrientation,
  type SplashScreen,
} from "@/lib/pwa";

const SPLASH_DIR = path.resolve(import.meta.dirname, "../../../public/splash");
const ORIENTATIONS: SplashOrientation[] = ["portrait", "landscape"];
const PNG_SIGNATURE = "89504e470d0a1a0a";

// Apple screen dimensions in portrait CSS points and device pixel ratio.
// Keep this table independent from lib/pwa-splash-screens.json so a wrong
// value there cannot make the assertions pass by construction.
const EXPECTED_SCREENS: SplashScreen[] = [
  { name: "splash-iphone-17-pro-max", width: 440, height: 956, scale: 3 },
  { name: "splash-iphone-air", width: 420, height: 912, scale: 3 },
  { name: "splash-iphone-17-pro", width: 402, height: 874, scale: 3 },
  { name: "splash-iphone-15-pro-max", width: 430, height: 932, scale: 3 },
  { name: "splash-iphone-15-pro", width: 393, height: 852, scale: 3 },
  { name: "splash-iphone-14-plus", width: 428, height: 926, scale: 3 },
  { name: "splash-iphone-14", width: 390, height: 844, scale: 3 },
  { name: "splash-iphone-x", width: 375, height: 812, scale: 3 },
  { name: "splash-iphone-xs-max", width: 414, height: 896, scale: 3 },
  { name: "splash-iphone-xr", width: 414, height: 896, scale: 2 },
  { name: "splash-iphone-8-plus", width: 414, height: 736, scale: 3 },
  { name: "splash-iphone-8", width: 375, height: 667, scale: 2 },
  { name: "splash-iphone-se", width: 320, height: 568, scale: 2 },
  { name: "splash-ipad-pro-13", width: 1032, height: 1376, scale: 2 },
  { name: "splash-ipad-pro-12", width: 1024, height: 1366, scale: 2 },
  { name: "splash-ipad-pro-11-m4", width: 834, height: 1210, scale: 2 },
  { name: "splash-ipad-pro-11", width: 834, height: 1194, scale: 2 },
  { name: "splash-ipad-air-11", width: 820, height: 1180, scale: 2 },
  { name: "splash-ipad-10-5", width: 834, height: 1112, scale: 2 },
  { name: "splash-ipad-10-2", width: 810, height: 1080, scale: 2 },
  { name: "splash-ipad-mini", width: 744, height: 1133, scale: 2 },
  { name: "splash-ipad", width: 768, height: 1024, scale: 2 },
];

function pngDimensions(buffer: Buffer): { width: number; height: number } {
  if (buffer.length < 8 || buffer.subarray(0, 8).toString("hex") !== PNG_SIGNATURE) {
    throw new Error("Not a PNG image");
  }
  if (buffer.length < 24) {
    throw new Error("Truncated PNG image");
  }
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}

describe("iOS splash screens", () => {
  it("declares the Apple mobile web app meta tag iOS needs for startup images", () => {
    expect(APPLE_WEB_APP_META).toEqual({ "apple-mobile-web-app-capable": "yes" });
    expect(metadata.other).toMatchObject({ "apple-mobile-web-app-capable": "yes" });
  });

  it("covers the reported iPhone 15 Pro and current iPad sizes", () => {
    expect(SPLASH_SCREENS).toEqual(EXPECTED_SCREENS);
    expect(SPLASH_SCREENS).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ name: "splash-iphone-15-pro", width: 393, height: 852, scale: 3 }),
        expect.objectContaining({ name: "splash-ipad-air-11", width: 820, height: 1180, scale: 2 }),
        expect.objectContaining({ name: "splash-ipad-pro-13", width: 1032, height: 1376, scale: 2 }),
        expect.objectContaining({ name: "splash-ipad-mini", width: 744, height: 1133, scale: 2 }),
      ])
    );
  });

  it("builds one portrait and one landscape link per screen", () => {
    expect(SPLASH_LINKS).toHaveLength(SPLASH_SCREENS.length * 2);

    for (const screen of SPLASH_SCREENS) {
      for (const orientation of ORIENTATIONS) {
        expect(SPLASH_LINKS).toContainEqual({
          src: splashAssetPath(screen, orientation),
          media: splashMedia(screen, orientation),
        });
        expect(splashMedia(screen, orientation)).toBe(
          `screen and (device-width: ${screen.width}px) and (device-height: ${screen.height}px) ` +
            `and (-webkit-device-pixel-ratio: ${screen.scale}) and (orientation: ${orientation})`
        );
      }
    }
  });

  it("returns no links when there is no screen to cover", () => {
    expect(buildSplashLinks([])).toEqual([]);
  });

  it("ships every declared image with the exact expected pixel dimensions", () => {
    for (const screen of SPLASH_SCREENS) {
      for (const orientation of ORIENTATIONS) {
        const src = splashAssetPath(screen, orientation);
        const file = path.join(SPLASH_DIR, path.basename(src));
        const dimensions = pngDimensions(readFileSync(file));
        const portraitWidth = screen.width * screen.scale;
        const portraitHeight = screen.height * screen.scale;

        expect({ file, ...dimensions }).toEqual({
          file,
          width: orientation === "portrait" ? portraitWidth : portraitHeight,
          height: orientation === "portrait" ? portraitHeight : portraitWidth,
        });
      }
    }
  });

  it("leaves no orphan image in public/splash", () => {
    const expected = new Set(SPLASH_LINKS.map((link) => path.basename(link.src)));
    const images = readdirSync(SPLASH_DIR).filter((file) => file.endsWith(".png"));
    expect(new Set(images)).toEqual(expected);
  });

  it("uses a unique media query per link", () => {
    const mediaQueries = SPLASH_LINKS.map((link) => link.media);
    expect(new Set(mediaQueries).size).toBe(mediaQueries.length);
  });

  it("rejects non-PNG and truncated images", () => {
    expect(() => pngDimensions(Buffer.alloc(24))).toThrow("Not a PNG image");
    expect(() => pngDimensions(Buffer.from(PNG_SIGNATURE, "hex"))).toThrow(
      "Truncated PNG image"
    );
  });
});
