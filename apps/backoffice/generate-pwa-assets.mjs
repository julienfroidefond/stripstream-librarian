import sharp from "sharp";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SOURCE = path.join(__dirname, "public/logostrip3.png");
const ICONS_DIR = path.join(__dirname, "public/icons");
const SPLASH_DIR = path.join(__dirname, "public/splash");
const BG_COLOR = { r: 15, g: 23, b: 42, alpha: 1 }; // #0f172a

// --- Icons ---
const iconSizes = [72, 96, 128, 144, 152, 180, 192, 384, 512];
const appleIconSizes = [
  { size: 180, name: "apple-touch-icon.png" },
  { size: 120, name: "apple-touch-icon-120.png" },
  { size: 152, name: "apple-touch-icon-152.png" },
  { size: 167, name: "apple-touch-icon-167.png" },
];

// --- Splash screens ---
const splashScreens = [
  { name: "splash-iphone-15-pro-max", w: 1290, h: 2796 },
  { name: "splash-iphone-15-pro", w: 1179, h: 2556 },
  { name: "splash-iphone-14-plus", w: 1284, h: 2778 },
  { name: "splash-iphone-14", w: 1170, h: 2532 },
  { name: "splash-iphone-x", w: 1125, h: 2436 },
  { name: "splash-iphone-xs-max", w: 1242, h: 2688 },
  { name: "splash-iphone-xr", w: 828, h: 1792 },
  { name: "splash-iphone-8-plus", w: 1242, h: 2208 },
  { name: "splash-iphone-8", w: 750, h: 1334 },
  { name: "splash-iphone-se", w: 640, h: 1136 },
  { name: "splash-ipad-pro-12", w: 2048, h: 2732 },
  { name: "splash-ipad-pro-11", w: 1668, h: 2388 },
  { name: "splash-ipad-10", w: 1668, h: 2224 },
  { name: "splash-ipad", w: 1536, h: 2048 },
];

async function generateIcons() {
  console.log("Generating icons...");
  const source = sharp(SOURCE);

  for (const size of iconSizes) {
    await source
      .clone()
      .resize(size, size, { fit: "contain", background: BG_COLOR })
      .png()
      .toFile(path.join(ICONS_DIR, `icon-${size}.png`));
    console.log(`  icon-${size}.png`);
  }

  // Maskable icon: 512x512 with 20% padding (safe zone)
  const maskableSize = 512;
  const logoSize = Math.round(maskableSize * 0.7);
  const logoBuf = await sharp(SOURCE).resize(logoSize, logoSize, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();
  await sharp({
    create: { width: maskableSize, height: maskableSize, channels: 4, background: BG_COLOR },
  })
    .composite([{ input: logoBuf, gravity: "centre" }])
    .png()
    .toFile(path.join(ICONS_DIR, "icon-maskable-512.png"));
  console.log("  icon-maskable-512.png");

  // Apple touch icons
  for (const { size, name } of appleIconSizes) {
    await source
      .clone()
      .resize(size, size, { fit: "contain", background: BG_COLOR })
      .png()
      .toFile(path.join(ICONS_DIR, name));
    console.log(`  ${name}`);
  }

  // Favicon (32x32)
  await source
    .clone()
    .resize(32, 32, { fit: "contain", background: BG_COLOR })
    .png()
    .toFile(path.join(ICONS_DIR, "favicon.png"));
  console.log("  favicon.png");
}

// Create a radial gradient background SVG matching the logo's cyan/magenta palette
function makeGradientSvg(w, h) {
  // Radial gradient: center glow with cyan and magenta hints on dark base
  return Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">
  <defs>
    <radialGradient id="bg" cx="50%" cy="50%" r="70%" fx="50%" fy="50%">
      <stop offset="0%" stop-color="#1a2744"/>
      <stop offset="40%" stop-color="#132035"/>
      <stop offset="100%" stop-color="#0a101e"/>
    </radialGradient>
    <radialGradient id="glow-cyan" cx="30%" cy="35%" r="50%">
      <stop offset="0%" stop-color="#00e5cc" stop-opacity="0.15"/>
      <stop offset="100%" stop-color="#00e5cc" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="glow-magenta" cx="70%" cy="65%" r="50%">
      <stop offset="0%" stop-color="#e0447a" stop-opacity="0.12"/>
      <stop offset="100%" stop-color="#e0447a" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect width="${w}" height="${h}" fill="url(#bg)"/>
  <rect width="${w}" height="${h}" fill="url(#glow-cyan)"/>
  <rect width="${w}" height="${h}" fill="url(#glow-magenta)"/>
</svg>`);
}

// Crop the logo into a circle
async function makeCircleLogo(size) {
  const logoResized = await sharp(SOURCE)
    .resize(size, size, { fit: "cover" })
    .png()
    .toBuffer();

  const circleMask = Buffer.from(
    `<svg width="${size}" height="${size}"><circle cx="${size / 2}" cy="${size / 2}" r="${size / 2}" fill="white"/></svg>`
  );

  // Circle mask with built-in opacity (0.7 = 70% opaque)
  const circleMaskAlpha = Buffer.from(
    `<svg width="${size}" height="${size}"><circle cx="${size / 2}" cy="${size / 2}" r="${size / 2}" fill="white" opacity="0.7"/></svg>`
  );

  return sharp(logoResized)
    .composite([{ input: circleMaskAlpha, blend: "dest-in" }])
    .png()
    .toBuffer();
}

// Create a subtle soft glow behind the circular logo
function makeGlowSvg(logoSize) {
  const glowSize = Math.round(logoSize * 1.6);
  const cx = glowSize / 2;
  return {
    svg: Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="${glowSize}" height="${glowSize}">
  <defs>
    <radialGradient id="g" cx="50%" cy="50%" r="50%">
      <stop offset="40%" stop-color="#00e5cc" stop-opacity="0.20"/>
      <stop offset="70%" stop-color="#0f172a" stop-opacity="0.04"/>
      <stop offset="100%" stop-color="#0f172a" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <circle cx="${cx}" cy="${cx}" r="${cx}" fill="url(#g)"/>
</svg>`),
    size: glowSize,
  };
}

async function generateSplash(name, w, h) {
  const shortDim = Math.min(w, h);
  const logoSize = Math.round(shortDim * 0.35);
  const circleLogoBuf = await makeCircleLogo(logoSize);
  const { svg: glowSvg, size: glowSize } = makeGlowSvg(logoSize);
  const glowBuf = await sharp(glowSvg).png().toBuffer();

  // Portrait
  const bgPortrait = makeGradientSvg(w, h);
  await sharp(bgPortrait)
    .composite([
      { input: glowBuf, gravity: "centre" },
      { input: circleLogoBuf, gravity: "centre" },
    ])
    .png()
    .toFile(path.join(SPLASH_DIR, `${name}-${w}x${h}.png`));
  console.log(`  ${name}-${w}x${h}.png`);

  // Landscape
  const bgLandscape = makeGradientSvg(h, w);
  await sharp(bgLandscape)
    .composite([
      { input: glowBuf, gravity: "centre" },
      { input: circleLogoBuf, gravity: "centre" },
    ])
    .png()
    .toFile(path.join(SPLASH_DIR, `${name}-ls-${h}x${w}.png`));
  console.log(`  ${name}-ls-${h}x${w}.png`);
}

async function generateSplashScreens() {
  console.log("Generating splash screens...");
  for (const { name, w, h } of splashScreens) {
    await generateSplash(name, w, h);
  }
}

async function main() {
  await generateIcons();
  await generateSplashScreens();
  console.log("\nDone! All PWA assets regenerated.");
}

main().catch(console.error);
