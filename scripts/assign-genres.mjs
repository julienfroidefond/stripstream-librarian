#!/usr/bin/env node
// Assigns genres to untagged series using Claude to pick from existing genres.
// Usage: node scripts/assign-genres.mjs [--dry-run]

import { readFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const DRY_RUN = process.argv.includes("--dry-run");

// ── Load credentials ──────────────────────────────────────────────────────────
const env = Object.fromEntries(
  readFileSync(join(__dirname, ".env"), "utf8")
    .split("\n")
    .filter((l) => l.includes("=") && !l.startsWith("#"))
    .map((l) => {
      const i = l.indexOf("=");
      return [l.slice(0, i).trim(), l.slice(i + 1).trim()];
    })
);

const API_URL = env.STRIPSTREAM_API_URL;
const API_TOKEN = env.STRIPSTREAM_API_TOKEN;
const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY;

if (!API_URL || !API_TOKEN) throw new Error("Missing STRIPSTREAM_API_URL or STRIPSTREAM_API_TOKEN in scripts/.env");
if (!ANTHROPIC_KEY) throw new Error("Missing ANTHROPIC_API_KEY in environment");

const apiHeaders = {
  Authorization: `Bearer ${API_TOKEN}`,
  "Content-Type": "application/json",
};

// ── Helpers ───────────────────────────────────────────────────────────────────
async function apiFetch(path, options = {}) {
  const res = await fetch(`${API_URL}${path}`, { ...options, headers: { ...apiHeaders, ...options.headers } });
  if (!res.ok) throw new Error(`${options.method ?? "GET"} ${path} → ${res.status} ${await res.text()}`);
  return res.json();
}

async function classifyGenres(seriesName, description, availableGenres) {
  const res = await fetch("https://api.anthropic.com/v1/messages", {
    method: "POST",
    headers: {
      "x-api-key": ANTHROPIC_KEY,
      "anthropic-version": "2023-06-01",
      "content-type": "application/json",
    },
    body: JSON.stringify({
      model: "claude-haiku-4-5-20251001",
      max_tokens: 256,
      messages: [
        {
          role: "user",
          content: `Tu dois classer une série de bande dessinée/manga. Choisis les genres les plus pertinents parmi la liste disponible. Réponds UNIQUEMENT avec un tableau JSON, rien d'autre.

Série : "${seriesName}"
Description : "${description ?? "Pas de description disponible"}"

Genres disponibles : ${JSON.stringify(availableGenres)}

Retourne un tableau JSON avec 1 à 4 genres maximum. Utilise UNIQUEMENT des genres présents dans la liste ci-dessus.`,
        },
      ],
    }),
  });

  const data = await res.json();
  if (!data.content?.[0]?.text) throw new Error(`Anthropic error: ${JSON.stringify(data)}`);

  const text = data.content[0].text.trim();
  const match = text.match(/\[[\s\S]*\]/);
  if (!match) throw new Error(`Unparseable response: ${text}`);

  const parsed = JSON.parse(match[0]);
  return parsed.filter((g) => availableGenres.includes(g));
}

// ── Main ──────────────────────────────────────────────────────────────────────
console.log(`🚀 assign-genres${DRY_RUN ? " (dry-run)" : ""}\n`);

const genres = await apiFetch("/genres");
const genreNames = genres.map((g) => g.name).sort();
console.log(`📚 ${genreNames.length} genres disponibles: ${genreNames.join(", ")}\n`);

const untagged = await apiFetch("/genres/untagged-series");
console.log(`🔍 ${untagged.length} séries sans genre\n`);

if (untagged.length === 0) {
  console.log("Rien à faire.");
  process.exit(0);
}

let updated = 0;
let skipped = 0;
let errors = 0;

for (const series of untagged) {
  try {
    const meta = await apiFetch(`/series/${series.series_id}/metadata`);
    const suggested = await classifyGenres(meta.series_name, meta.description, genreNames);

    if (suggested.length === 0) {
      console.log(`⚠️  ${meta.series_name} → aucun genre identifié`);
      skipped++;
      continue;
    }

    if (DRY_RUN) {
      console.log(`🔵 [dry] ${meta.series_name} → [${suggested.join(", ")}]`);
      updated++;
      continue;
    }

    await apiFetch(`/series/${series.series_id}`, {
      method: "PATCH",
      body: JSON.stringify({
        new_name: meta.series_name,
        genres: suggested,
        authors: meta.authors,
        publishers: meta.publishers,
        description: meta.description ?? undefined,
        start_year: meta.start_year ?? undefined,
        total_volumes: meta.total_volumes ?? undefined,
        status: meta.status ?? undefined,
      }),
    });

    console.log(`✅ ${meta.series_name} → [${suggested.join(", ")}]`);
    updated++;

    // Avoid hammering the API + Anthropic rate limits
    await new Promise((r) => setTimeout(r, 300));
  } catch (err) {
    console.error(`❌ ${series.name}: ${err.message}`);
    errors++;
  }
}

console.log(`\n── Résultat ──`);
console.log(`  ✅ ${updated} mis à jour`);
console.log(`  ⚠️  ${skipped} ignorés (aucun genre identifié)`);
console.log(`  ❌ ${errors} erreurs`);
