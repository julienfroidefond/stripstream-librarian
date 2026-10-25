import splashScreensData from "./pwa-splash-screens.json";

export interface SplashScreen {
  /** File-name prefix, e.g. `splash-iphone-15-pro`. */
  name: string;
  /** Portrait screen width in CSS points. */
  width: number;
  /** Portrait screen height in CSS points. */
  height: number;
  /** Device pixel ratio (`-webkit-device-pixel-ratio`). */
  scale: number;
}

export type SplashOrientation = "portrait" | "landscape";

export interface SplashLink {
  src: string;
  media: string;
}

export const SPLASH_SCREENS: SplashScreen[] = splashScreensData;

export function splashAssetPath(screen: SplashScreen, orientation: SplashOrientation): string {
  const portraitWidth = screen.width * screen.scale;
  const portraitHeight = screen.height * screen.scale;
  const suffix = orientation === "portrait" ? "" : "-ls";
  const dimensions =
    orientation === "portrait"
      ? `${portraitWidth}x${portraitHeight}`
      : `${portraitHeight}x${portraitWidth}`;
  return `/splash/${screen.name}${suffix}-${dimensions}.png`;
}

export function splashMedia(screen: SplashScreen, orientation: SplashOrientation): string {
  return [
    "screen",
    `(device-width: ${screen.width}px)`,
    `(device-height: ${screen.height}px)`,
    `(-webkit-device-pixel-ratio: ${screen.scale})`,
    `(orientation: ${orientation})`,
  ].join(" and ");
}

export function buildSplashLinks(screens: SplashScreen[]): SplashLink[] {
  return screens.flatMap((screen) =>
    (["portrait", "landscape"] as const).map((orientation) => ({
      src: splashAssetPath(screen, orientation),
      media: splashMedia(screen, orientation),
    }))
  );
}

export const SPLASH_LINKS: SplashLink[] = buildSplashLinks(SPLASH_SCREENS);

// Next.js only emits `mobile-web-app-capable` for `appleWebApp.capable`, but
// iOS still requires the Apple-prefixed tag to display startup images.
export const APPLE_WEB_APP_META = {
  "apple-mobile-web-app-capable": "yes",
} as const;
