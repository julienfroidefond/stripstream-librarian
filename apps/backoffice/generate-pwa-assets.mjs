import sharp from "sharp";
import { writeFile } from "fs/promises";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PUBLIC_DIR = path.join(__dirname, "public");
// One logo per use: the dark one ("black") for app icons and splash screens —
// it reads better on a home screen than the paper one — and the monochrome one
// for favicons (the only one legible that small).
const SOURCE = path.join(PUBLIC_DIR, "logo-dark.webp");
const SPLASH_SOURCE = path.join(PUBLIC_DIR, "logo-dark.webp");
const FAVICON_SOURCE = path.join(PUBLIC_DIR, "logo-mono.webp");
const ICONS_DIR = path.join(PUBLIC_DIR, "icons");
const SPLASH_DIR = path.join(PUBLIC_DIR, "splash");
// Background of the dark logo (sampled from its top-left pixel), so the
// maskable icon and the splash screens blend seamlessly with it.
const DARK_BG_COLOR = { r: 12, g: 10, b: 8, alpha: 1 }; // #0c0a08

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

// An .ico file holding a single PNG image (supported by every modern browser).
function pngToIco(png, size) {
  const header = Buffer.alloc(22);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type: icon
  header.writeUInt16LE(1, 4); // image count
  header.writeUInt8(size, 6);
  header.writeUInt8(size, 7);
  header.writeUInt16LE(1, 10); // color planes
  header.writeUInt16LE(32, 12); // bits per pixel
  header.writeUInt32LE(png.length, 14);
  header.writeUInt32LE(22, 18); // image offset
  return Buffer.concat([header, png]);
}

async function generateIcons() {
  console.log("Generating icons...");
  const source = sharp(SOURCE);

  for (const size of iconSizes) {
    await source
      .clone()
      .resize(size, size)
      .png()
      .toFile(path.join(ICONS_DIR, `icon-${size}.png`));
    console.log(`  icon-${size}.png`);
  }

  // Maskable icon: 512x512 with the logo shrunk into the 80% safe zone
  const maskableSize = 512;
  const logoSize = Math.round(maskableSize * 0.7);
  const logoBuf = await sharp(SOURCE).resize(logoSize, logoSize).png().toBuffer();
  await sharp({
    create: { width: maskableSize, height: maskableSize, channels: 4, background: DARK_BG_COLOR },
  })
    .composite([{ input: logoBuf, gravity: "centre" }])
    .png()
    .toFile(path.join(ICONS_DIR, "icon-maskable-512.png"));
  console.log("  icon-maskable-512.png");

  // Apple touch icons
  for (const { size, name } of appleIconSizes) {
    await source
      .clone()
      .resize(size, size)
      .png()
      .toFile(path.join(ICONS_DIR, name));
    console.log(`  ${name}`);
  }
  await source.clone().resize(180, 180).png().toFile(path.join(PUBLIC_DIR, "apple-touch-icon.png"));
  console.log("  ../apple-touch-icon.png");

  // Favicons (32x32)
  const favicon = await sharp(FAVICON_SOURCE).resize(32, 32).png().toBuffer();
  await writeFile(path.join(ICONS_DIR, "favicon.png"), favicon);
  console.log("  favicon.png");
  await writeFile(path.join(PUBLIC_DIR, "favicon.ico"), pngToIco(favicon, 32));
  console.log("  ../favicon.ico");
}

async function generateSplash(name, w, h) {
  const logoSize = Math.round(Math.min(w, h) * 0.4);
  const logoBuf = await sharp(SPLASH_SOURCE).resize(logoSize, logoSize).png().toBuffer();

  for (const [width, height, file] of [
    [w, h, `${name}-${w}x${h}.png`],
    [h, w, `${name}-ls-${h}x${w}.png`],
  ]) {
    await sharp({ create: { width, height, channels: 4, background: DARK_BG_COLOR } })
      .composite([{ input: logoBuf, gravity: "centre" }])
      .png()
      .toFile(path.join(SPLASH_DIR, file));
    console.log(`  ${file}`);
  }
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
