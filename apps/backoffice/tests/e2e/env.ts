import fs from "node:fs";
import path from "node:path";

/** Minimal `.env.local` parser (no dependency on dotenv). */
export function loadEnvLocal(): Record<string, string> {
  const env: Record<string, string> = {};
  const file = path.resolve(__dirname, "../../.env.local");
  if (!fs.existsSync(file)) return env;

  for (const rawLine of fs.readFileSync(file, "utf8").split("\n")) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const match = line.match(/^([A-Za-z0-9_]+)\s*=\s*(.*)$/);
    if (!match) continue;
    env[match[1]] = match[2].trim().replace(/^["']|["']$/g, "");
  }
  return env;
}

export function getAdminCredentials(): { username: string; password: string } {
  const env = loadEnvLocal();
  const username = process.env.ADMIN_USERNAME || env.ADMIN_USERNAME || "admin";
  const password = process.env.ADMIN_PASSWORD || env.ADMIN_PASSWORD;
  if (!password) {
    throw new Error(
      "ADMIN_PASSWORD introuvable. Ajoute-le à apps/backoffice/.env.local ou passe-le en variable d'environnement."
    );
  }
  return { username, password };
}
