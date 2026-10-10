# REFACTOR TODO — Factorisations

Checklist des opportunités de factorisation identifiées (Rust + Backoffice), triées par
priorité. Chaque lot est indépendant et livrable avec ses tests.

> **État :** Front / Backoffice — ✅ **terminé** (mergé via #54, v3.32.5/3.32.6).
> Back / Rust — ✅ **terminé** (mergé via #56, v3.32.7).

**Conventions :** cocher `[x]` quand la tâche est terminée et testée.
**Rappel tests :** `cargo test --workspace` (Rust) — `cd apps/backoffice && npm run test:unit &&
npx tsc --noEmit && npm run lint` (front).

---

## P0 — Gain immédiat, risque quasi nul

### P0.1 — Rust : extraire les helpers providers dupliqués
- [x] Créer `compute_confidence(title, query) -> f32` dans `metadata_providers/mod.rs`
      (logique identique dans `open_library.rs:454`, `comicvine.rs:362`,
      `google_books.rs:554`, `anilist.rs:532`)
- [x] Créer `urlencoded(s) -> String` dans `metadata_providers/mod.rs`
      (copies : `open_library.rs:469`, `comicvine.rs:377`, `google_books.rs:570`)
- [x] Créer `strip_html(s) -> String` dans `metadata_providers/mod.rs`
      (copie unique : `comicvine.rs:348`)
- [x] Supprimer les 4 copies locales de `compute_confidence` et importer le helper partagé
- [x] Supprimer les 3 copies locales de `urlencoded` et importer le helper partagé
- [x] Déplacer `strip_html` dans le module partagé (garder les tests `comicvine`)
- [x] Tests unitaires du helper partagé (couvrir exactitude, prefix, contains, overlap)
- [x] `cargo test --workspace` vert

### P0.2 — Rust : simplifier `EventToggles`
- [x] Remplacer les 21 `#[serde(default = "default_true")]` par `#[serde(default)]` +
      `impl Default for EventToggles` (`crates/notifications/src/lib.rs:20-64`)
- [x] Supprimer `default_true()` (`lib.rs:66`) et `default_events()` (`lib.rs:70`)
- [x] Passer `TelegramConfig.events` à `#[serde(default)]` (`lib.rs:16-17`)
- [x] Vérifier que le parsing d'un JSON `telegram` partiel active toujours tous les events
- [x] Test : désérialisation avec/sans bloc `events`
- [x] `cargo test -p notifications` (ou `--workspace`) vert

### P0.3 — Front : sanitiser les `dangerouslySetInnerHTML` — ✅
- [x] `TelegramCard.tsx:73,77,81` → utiliser `SafeHtml`
- [x] `SettingsPage.tsx:348` → utiliser `SafeHtml`
- [x] Vérifier que le rendu HTML (gras, liens) reste correct après sanitisation
- [x] Tests unitaires des cartes concernées verts
- [x] `npx tsc --noEmit` + `npm run lint` verts

---

## P1 — Gain élevé, risque faible

### P1.1 — Rust : helper de lecture `app_settings` (~20 occurrences)
- [x] Ajouter `load_setting<T: DeserializeOwned>(pool, key) -> Result<Option<T>>` (et
      variante requise `load_setting_or_default`)
- [x] Remplacer les lectures dans `settings.rs:113,460`
- [x] Remplacer les lectures dans `state.rs:155,175,195,210,229`
- [x] Remplacer les lectures dans `metadata/config.rs:19,53` et `metadata/handlers.rs:840`
- [x] Remplacer les lectures dans `downloads/qbittorrent.rs:53`, `downloads/prowlarr.rs:107`,
      `downloads/telegram_monitor.rs:147`, `downloads/torrent_import.rs:639`
- [x] Remplacer les lectures dans `ai_tagging.rs:109`, `books/mod.rs:773`, `books/rename.rs:166`
- [x] Remplacer les lectures dans `integrations/anilist.rs:65`,
      `integrations/discovery/mod.rs:466`
- [x] Unifier `crates/notifications/src/lib.rs:98` sur le helper (si partageable)
- [x] Tests unitaires du helper + `cargo test --workspace` vert

### P1.2 — Rust : dédupliquer `normalize_lexically`
- [x] Déplacer la fonction dans `crates/core` (module chemins)
- [x] Supprimer la copie `settings.rs:220`
- [x] Supprimer la copie `downloads/torrent_import.rs:106`
- [x] Importer le helper depuis `core` dans les deux modules
- [x] Tests existants verts + `cargo test --workspace`

### P1.3 — Front : hook `useMarkRead` — ✅
- [x] Créer `hooks/useMarkRead.ts` : `{ loading, handleClick }` (preventDefault,
      stopPropagation, fetch, gestion `!res.ok`, `router.refresh`, reset loading)
- [x] Extraire `markReadClassName(completed, compact)` (styles partagés)
- [x] Refactorer `MarkBookReadButton.tsx` pour l'utiliser
- [x] Refactorer `MarkSeriesReadButton.tsx` pour l'utiliser
- [x] Conserver l'API publique et labels actuels (`markRead.*`)
- [x] Tests `MarkBookReadButton` / `MarkSeriesReadButton` verts (+ test du hook)

### P1.4 — Front : uniformiser les imports i18n (61 occurrences) — ✅
- [x] Remplacer `from "../../lib/i18n/context"` par `from "@/lib/i18n/context"` partout
- [x] Vérifier l'absence de chemins relatifs résiduels (`grep "\.\./.*lib/i18n/context"`)
- [x] `npx tsc --noEmit` + `npm run lint` verts

---

## P2 — Gain élevé (volume), risque moyen

### P2.1 — Rust : helpers de cycle de vie des jobs
- [x] Créer `job_in_flight(pool, library_id: Option<Uuid>, types: &[&str]) -> Result<Option<Uuid>>`
      (factorise ~17 checks `status IN ('pending','running')`)
- [x] Créer `fail_job(pool, job_id, error)` — gérer la variante **avec** et **sans** `stats_json`
      (16 occurrences de `UPDATE ... 'failed'`)
- [x] Créer `complete_job(pool, job_id, stats)` (14 occurrences de `UPDATE ... 'success'`)
- [x] Migrer `jobs/index_jobs.rs`
- [x] Migrer `metadata/batch.rs`
- [x] Migrer `metadata/refresh.rs`
- [x] Migrer `downloads/detection.rs`
- [x] Migrer `downloads/rss_poll.rs`
- [x] Migrer `downloads/telegram_monitor.rs`
- [x] Migrer `reading/status_match.rs`
- [x] Migrer `reading/status_push.rs`
- [x] Migrer `reading/status_pull.rs`
- [x] Migrer `jobs/poller.rs`
- [x] Tests `#[sqlx::test]` couvrant `job_in_flight` / `fail_job` / `complete_job`
- [x] `cargo test --workspace` vert

### P2.2 — Rust : mutualiser le SQL `index_jobs`
- [x] Extraire `const JOB_SELECT` (+ variante détail) dans `jobs/index_jobs.rs`
- [x] Remplacer les 5 requêtes dupliquées (`index_jobs.rs:158,177,202,242,432`)
- [x] Unifier `map_row` / `map_row_detail` sur les colonnes réellement sélectionnées
- [x] Vérifier les endpoints jobs (liste, détail, replay) via tests
- [x] `cargo test --workspace` vert

### P2.3 — Front : `withRoute` pour les route handlers (~100 fichiers) — ✅
- [x] Créer `lib/api-handler.ts` (`withRoute`, extraction message d'erreur, statut 500 par défaut)
- [x] Supporter un statut/`fallback` personnalisé (ex. 400 « id is required », 404)
- [x] Migrer les handlers `apps/backoffice/app/api/**` (codemod puis revue manuelle)
- [x] Conserver les statuts spécifiques (`series/[seriesId]/rename-books`, `books/[bookId]/convert`,
      `telegram-monitor`)
- [x] `tests/unit/app/api/errors.test.ts` + suite complète verte
- [x] `npx tsc --noEmit` + `npm run lint` verts

---

## P3 — Gain moyen, risque faible

### P3.1 — Front : composant `SettingsCard` / usages de `FormLabel` — ✅
- [x] Aligner `FormLabel` (`ui/Form.tsx:19`) sur le style settings (ou variante)
- [x] Créer `SettingsCard` (Card + CardHeader icône/titre/description + CardContent)
- [x] Créer `SettingsField` (label + `FormInput`/`FormSelect` avec save onBlur)
- [x] Migrer les 45 labels inline : `SettingsPage.tsx`, `TelegramCard`, `QBittorrentCard`,
      `ProwlarrCard`, `KomgaSyncCard`, `AiTaggingCard`, `AnilistTab`, `TelegramMonitorCard`,
      `RenameFormatCard`
- [x] Tests des cartes settings verts
- [x] `npx tsc --noEmit` + `npm run lint` verts

### P3.2 — Front : composant `Switch` — ✅
- [x] Créer `ui/Switch.tsx` (pattern `sr-only peer` réutilisable)
- [x] Exporter depuis `ui/index.ts`
- [x] Remplacer les toggles `TelegramCard.tsx:96,206`
- [x] Rechercher d'autres toggles dupliqués et migrer
- [x] Tests du Switch + `npx tsc --noEmit` verts

### P3.3 — Front : grille de stats pour les rapports de jobs — ✅
- [x] Créer `ReportStatGrid` (grille de `StatBox` paramétrable)
- [x] Migrer `MetadataReportCards`
- [x] Migrer `ReadingStatusReportCards`
- [x] Migrer `DownloadDetectionCards`
- [x] Migrer `JobProgressCard`
- [x] Tests des cartes de rapport verts

---

## P4 — Gain moyen, risque plus élevé

### P4.1 — Rust : factory de clients HTTP (31 occurrences)
- [x] Créer `build_http_client(timeout)` / `HttpClientFactory` partagé
- [x] Migrer `metadata_providers/*`
- [x] Migrer `downloads/*`
- [x] Migrer `integrations/*`
- [x] Migrer `ai_tagging.rs`
- [x] Migrer `crates/notifications/src/lib.rs:120`
- [x] Homogénéiser timeouts et user-agent
- [x] `cargo test --workspace` vert

### P4.2 — Rust : scraping `bdtheque` / `bdphile`
- [x] Extraire `client()`, `text(el)`, `absolute(base, href)` dans un module de scraping commun
- [x] Refactorer `bdtheque.rs`
- [x] Refactorer `bdphile.rs`
- [x] Tests des deux providers verts

### P4.3 — Rust : boilerplate trait `MetadataProvider`
- [x] Évaluer macro / méthodes par défaut pour supprimer les 18 `Box::pin(async move {})`
      → retenu : `#[async_trait::async_trait]` (macro idiomatique, trait object-safe conservé)
- [x] Implémenter l'approche retenue (`async fn` dans le trait + impls)
- [x] Migrer les 6 providers (+ `anilist`)
- [x] `cargo test --workspace` vert

### P4.4 — Découpage des gros fichiers
- [x] `crates/parsers/src/lib.rs` (3301 l.) → `zip.rs`, `cbr.rs`, `pdf.rs`, `epub.rs`,
      `volume.rs`, `metadata.rs`
- [x] `apps/api/src/downloads/telegram_monitor.rs` (2544 l.) → auth / sync canaux / jobs
      (`telegram_monitor/{auth,channels,jobs,books,types}.rs`)
- [x] `apps/indexer/src/scanner.rs` (2283 l.) → discovery / mtime / classification volume_type
      (`scanner/{discovery,archive,mtime}.rs`)
- [x] `apps/api/src/stats.rs` (1571 l.) → agrégations par domaine
      (`stats/{types,period,get_stats,overview,breakdown,reading}.rs`)
- [x] `apps/backoffice/lib/api.ts` (2078 l.) → éclaté en `lib/api/*` (barrel `index.ts`)
- [x] `cargo test --workspace` vert après chaque découpage (Rust + front)

---

## Backlog — à évaluer plus tard

- [ ] Extraction de tous les helpers providers restants dans un module `text.rs` dédié
- [ ] Revue des autres `reqwest` non-builder / réutilisation de clients au sein d'un provider
- [ ] Audit global des autres usages `dangerouslySetInnerHTML` (hors settings)

---

## Journal de progression

| Lot | Statut | Date | Notes |
|-----|--------|------|-------|
| P0.1 | ✅ | 2026-10-10 | Helpers providers partagés (`compute_confidence`, `urlencoded`, `strip_html`) + tests |
| P0.2 | ✅ | 2026-10-10 | `EventToggles` via `#[serde(default)]` + `impl Default`, tests désérialisation |
| P0.3 | ✅ | 2026-10-10 | #54 (front) |
| P1.1 | ✅ | 2026-10-10 | `load_setting`/`load_setting_or_default` dans `crates/core` + migration de ~20 sites |
| P1.2 | ✅ | 2026-10-10 | `normalize_lexically` déplacé dans `crates/core/src/paths.rs` |
| P1.3 | ✅ | 2026-10-10 | #54 (front) |
| P1.4 | ✅ | 2026-10-10 | #54 (front) |
| P2.1 | ✅ | 2026-10-10 | `jobs/lifecycle.rs` (`job_in_flight`/`fail_job`/`complete_job`) + migration + tests |
| P2.2 | ✅ | 2026-10-10 | Constantes `JOB_SELECT`/`JOB_DETAIL_SELECT` dans `jobs/index_jobs.rs` |
| P2.3 | ✅ | 2026-10-10 | #54 (front) |
| P3.1 | ✅ | 2026-10-10 | #54 (front) |
| P3.2 | ✅ | 2026-10-10 | #54 (front) |
| P3.3 | ✅ | 2026-10-10 | #54 (front) |
| P4.1 | ✅ | 2026-10-10 | `crates/core/src/http.rs` (`build_http_client`++) + migration ~25 sites, UA homogénéisé |
| P4.2 | ✅ | 2026-10-10 | `metadata_providers/scraping.rs` (`client`/`text`/`absolute`/`get_html`) + tests |
| P4.3 | ✅ | 2026-10-10 | `#[async_trait::async_trait]` sur le trait + 7 providers, 21 `Box::pin` supprimés |
| P4.4 | ✅ | 2026-10-10 | Découpage parsers + stats + scanner + telegram_monitor (#56) ; `lib/api.ts` éclaté (#54) |
