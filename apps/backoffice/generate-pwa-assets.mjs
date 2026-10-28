import sharp from "sharp";
import { readFile, readdir, unlink, writeFile } from "fs/promises";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PUBLIC_DIR = path.join(__dirname, "public");
// One logo per use: the dark mark for app icons and splash screens — it reads
// better on a home screen than the paper one — and the monochrome one for
// favicons (the only one legible that small). The marks are the cut-out
// artwork: with no baked-in background field, icons and splash screens can take
// the app background instead of the colour frozen into the old square.
const SOURCE = path.join(PUBLIC_DIR, "logo-mark-dark.png");
const SPLASH_SOURCE = path.join(PUBLIC_DIR, "logo-mark-dark.png");
const FAVICON_SOURCE = path.join(PUBLIC_DIR, "logo-mono.webp");
const ICONS_DIR = path.join(PUBLIC_DIR, "icons");
const SPLASH_DIR = path.join(PUBLIC_DIR, "splash");
// `--color-background` of the dark theme (hsl(220 13% 9%)), the colour of the
// room: maskable icon and splash screens blend into it without a visible square.
const THEME_BG_COLOR = { r: 20, g: 22, b: 26, alpha: 1 }; // #14161a

// --- Icons ---
const iconSizes = [72, 96, 128, 144, 152, 180, 192, 384, 512];
const appleIconSizes = [
  { size: 180, name: "apple-touch-icon.png" },
  { size: 120, name: "apple-touch-icon-120.png" },
  { size: 152, name: "apple-touch-icon-152.png" },
  { size: 167, name: "apple-touch-icon-167.png" },
];

// --- Splash screens ---
// Device specs live in lib/pwa-splash-screens.json, shared with the layout.
const SPLASH_SCREENS_FILE = new URL("./lib/pwa-splash-screens.json", import.meta.url);

async function loadSplashScreens() {
  return JSON.parse(await readFile(SPLASH_SCREENS_FILE, "utf8"));
}

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
      .flatten({ background: THEME_BG_COLOR }) // iOS rejects transparency
      .png()
      .toFile(path.join(ICONS_DIR, `icon-${size}.png`));
    console.log(`  icon-${size}.png`);
  }

  // Maskable icon: 512x512 with the logo shrunk into the 80% safe zone
  const maskableSize = 512;
  const logoSize = Math.round(maskableSize * 0.7);
  const logoBuf = await sharp(SOURCE).resize(logoSize, logoSize).png().toBuffer();
  await sharp({
    create: { width: maskableSize, height: maskableSize, channels: 4, background: THEME_BG_COLOR },
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
      .flatten({ background: THEME_BG_COLOR }) // iOS rejects transparency
      .png()
      .toFile(path.join(ICONS_DIR, name));
    console.log(`  ${name}`);
  }
  await source
    .clone()
    .resize(180, 180)
    .flatten({ background: THEME_BG_COLOR })
    .png()
    .toFile(path.join(PUBLIC_DIR, "apple-touch-icon.png"));
  console.log("  ../apple-touch-icon.png");

  // Favicons (32x32)
  const favicon = await sharp(FAVICON_SOURCE).resize(32, 32).png().toBuffer();
  await writeFile(path.join(ICONS_DIR, "favicon.png"), favicon);
  console.log("  favicon.png");
  await writeFile(path.join(PUBLIC_DIR, "favicon.ico"), pngToIco(favicon, 32));
  console.log("  ../favicon.ico");
}

function splashAsset(screen, orientation) {
  const portraitWidth = screen.width * screen.scale;
  const portraitHeight = screen.height * screen.scale;
  const isPortrait = orientation === "portrait";
  const width = isPortrait ? portraitWidth : portraitHeight;
  const height = isPortrait ? portraitHeight : portraitWidth;
  const suffix = isPortrait ? "" : "-ls";
  return { width, height, file: `${screen.name}${suffix}-${width}x${height}.png` };
}

async function generateSplash(screen) {
  const logoSize = Math.round(Math.min(screen.width, screen.height) * screen.scale * 0.4);
  const logoBuf = await sharp(SPLASH_SOURCE).resize(logoSize, logoSize).png().toBuffer();

  for (const orientation of ["portrait", "landscape"]) {
    const { width, height, file } = splashAsset(screen, orientation);
    await sharp({ create: { width, height, channels: 4, background: THEME_BG_COLOR } })
      .composite([{ input: logoBuf, gravity: "centre" }])
      .png()
      .toFile(path.join(SPLASH_DIR, file));
    console.log(`  ${file}`);
  }
}

async function generateSplashScreens() {
  console.log("Generating splash screens...");
  const screens = await loadSplashScreens();
  const expected = new Set(
    screens.flatMap((screen) => ["portrait", "landscape"].map((o) => splashAsset(screen, o).file))
  );

  for (const screen of screens) {
    await generateSplash(screen);
  }

  for (const file of await readdir(SPLASH_DIR)) {
    if (file.endsWith(".png") && !expected.has(file)) {
      await unlink(path.join(SPLASH_DIR, file));
      console.log(`  removed stale ${file}`);
    }
  }
}

async function main() {
  await generateIcons();
  await generateSplashScreens();
  console.log("\nDone! All PWA assets regenerated.");
}

main().catch(console.error);
