# REFACTOR TODO — Factorisations

Checklist des opportunités de factorisation identifiées (Rust + Backoffice), triées par
priorité. Chaque lot est indépendant et livrable avec ses tests.

**Conventions :** cocher `[x]` quand la tâche est terminée et testée.
**Rappel tests :** `cargo test --workspace` (Rust) — `cd apps/backoffice && npm run test:unit &&
npx tsc --noEmit && npm run lint` (front).

---

## P0 — Gain immédiat, risque quasi nul

### P0.1 — Rust : extraire les helpers providers dupliqués
- [ ] Créer `compute_confidence(title, query) -> f32` dans `metadata_providers/mod.rs`
      (logique identique dans `open_library.rs:454`, `comicvine.rs:362`,
      `google_books.rs:554`, `anilist.rs:532`)
- [ ] Créer `urlencoded(s) -> String` dans `metadata_providers/mod.rs`
      (copies : `open_library.rs:469`, `comicvine.rs:377`, `google_books.rs:570`)
- [ ] Créer `strip_html(s) -> String` dans `metadata_providers/mod.rs`
      (copie unique : `comicvine.rs:348`)
- [ ] Supprimer les 4 copies locales de `compute_confidence` et importer le helper partagé
- [ ] Supprimer les 3 copies locales de `urlencoded` et importer le helper partagé
- [ ] Déplacer `strip_html` dans le module partagé (garder les tests `comicvine`)
- [ ] Tests unitaires du helper partagé (couvrir exactitude, prefix, contains, overlap)
- [ ] `cargo test --workspace` vert

### P0.2 — Rust : simplifier `EventToggles`
- [ ] Remplacer les 21 `#[serde(default = "default_true")]` par `#[serde(default)]` +
      `impl Default for EventToggles` (`crates/notifications/src/lib.rs:20-64`)
- [ ] Supprimer `default_true()` (`lib.rs:66`) et `default_events()` (`lib.rs:70`)
- [ ] Passer `TelegramConfig.events` à `#[serde(default)]` (`lib.rs:16-17`)
- [ ] Vérifier que le parsing d'un JSON `telegram` partiel active toujours tous les events
- [ ] Test : désérialisation avec/sans bloc `events`
- [ ] `cargo test -p notifications` (ou `--workspace`) vert

### P0.3 — Front : sanitiser les `dangerouslySetInnerHTML`
- [ ] `TelegramCard.tsx:73,77,81` → utiliser `SafeHtml`
- [ ] `SettingsPage.tsx:348` → utiliser `SafeHtml`
- [ ] Vérifier que le rendu HTML (gras, liens) reste correct après sanitisation
- [ ] Tests unitaires des cartes concernées verts
- [ ] `npx tsc --noEmit` + `npm run lint` verts

---

## P1 — Gain élevé, risque faible

### P1.1 — Rust : helper de lecture `app_settings` (~20 occurrences)
- [ ] Ajouter `load_setting<T: DeserializeOwned>(pool, key) -> Result<Option<T>>` (et
      variante requise `load_setting_or_default`)
- [ ] Remplacer les lectures dans `settings.rs:113,460`
- [ ] Remplacer les lectures dans `state.rs:155,175,195,210,229`
- [ ] Remplacer les lectures dans `metadata/config.rs:19,53` et `metadata/handlers.rs:840`
- [ ] Remplacer les lectures dans `downloads/qbittorrent.rs:53`, `downloads/prowlarr.rs:107`,
      `downloads/telegram_monitor.rs:147`, `downloads/torrent_import.rs:639`
- [ ] Remplacer les lectures dans `ai_tagging.rs:109`, `books/mod.rs:773`, `books/rename.rs:166`
- [ ] Remplacer les lectures dans `integrations/anilist.rs:65`,
      `integrations/discovery/mod.rs:466`
- [ ] Unifier `crates/notifications/src/lib.rs:98` sur le helper (si partageable)
- [ ] Tests unitaires du helper + `cargo test --workspace` vert

### P1.2 — Rust : dédupliquer `normalize_lexically`
- [ ] Déplacer la fonction dans `crates/core` (module chemins)
- [ ] Supprimer la copie `settings.rs:220`
- [ ] Supprimer la copie `downloads/torrent_import.rs:106`
- [ ] Importer le helper depuis `core` dans les deux modules
- [ ] Tests existants verts + `cargo test --workspace`

### P1.3 — Front : hook `useMarkRead`
- [ ] Créer `hooks/useMarkRead.ts` : `{ loading, handleClick }` (preventDefault,
      stopPropagation, fetch, gestion `!res.ok`, `router.refresh`, reset loading)
- [ ] Extraire `markReadClassName(completed, compact)` (styles partagés)
- [ ] Refactorer `MarkBookReadButton.tsx` pour l'utiliser
- [ ] Refactorer `MarkSeriesReadButton.tsx` pour l'utiliser
- [ ] Conserver l'API publique et labels actuels (`markRead.*`)
- [ ] Tests `MarkBookReadButton` / `MarkSeriesReadButton` verts (+ test du hook)

### P1.4 — Front : uniformiser les imports i18n (61 occurrences)
- [ ] Remplacer `from "../../lib/i18n/context"` par `from "@/lib/i18n/context"` partout
- [ ] Vérifier l'absence de chemins relatifs résiduels (`grep "\.\./.*lib/i18n/context"`)
- [ ] `npx tsc --noEmit` + `npm run lint` verts

---

## P2 — Gain élevé (volume), risque moyen

### P2.1 — Rust : helpers de cycle de vie des jobs
- [ ] Créer `job_in_flight(pool, library_id: Option<Uuid>, types: &[&str]) -> Result<Option<Uuid>>`
      (factorise ~17 checks `status IN ('pending','running')`)
- [ ] Créer `fail_job(pool, job_id, error)` — gérer la variante **avec** et **sans** `stats_json`
      (16 occurrences de `UPDATE ... 'failed'`)
- [ ] Créer `complete_job(pool, job_id, stats)` (14 occurrences de `UPDATE ... 'success'`)
- [ ] Migrer `jobs/index_jobs.rs`
- [ ] Migrer `metadata/batch.rs`
- [ ] Migrer `metadata/refresh.rs`
- [ ] Migrer `downloads/detection.rs`
- [ ] Migrer `downloads/rss_poll.rs`
- [ ] Migrer `downloads/telegram_monitor.rs`
- [ ] Migrer `reading/status_match.rs`
- [ ] Migrer `reading/status_push.rs`
- [ ] Migrer `reading/status_pull.rs`
- [ ] Migrer `jobs/poller.rs`
- [ ] Tests `#[sqlx::test]` couvrant `job_in_flight` / `fail_job` / `complete_job`
- [ ] `cargo test --workspace` vert

### P2.2 — Rust : mutualiser le SQL `index_jobs`
- [ ] Extraire `const JOB_SELECT` (+ variante détail) dans `jobs/index_jobs.rs`
- [ ] Remplacer les 5 requêtes dupliquées (`index_jobs.rs:158,177,202,242,432`)
- [ ] Unifier `map_row` / `map_row_detail` sur les colonnes réellement sélectionnées
- [ ] Vérifier les endpoints jobs (liste, détail, replay) via tests
- [ ] `cargo test --workspace` vert

### P2.3 — Front : `withRoute` pour les route handlers (~100 fichiers)
- [ ] Créer `lib/api-handler.ts` (`withRoute`, extraction message d'erreur, statut 500 par défaut)
- [ ] Supporter un statut/`fallback` personnalisé (ex. 400 « id is required », 404)
- [ ] Migrer les handlers `apps/backoffice/app/api/**` (codemod puis revue manuelle)
- [ ] Conserver les statuts spécifiques (`series/[seriesId]/rename-books`, `books/[bookId]/convert`,
      `telegram-monitor`)
- [ ] `tests/unit/app/api/errors.test.ts` + suite complète verte
- [ ] `npx tsc --noEmit` + `npm run lint` verts

---

## P3 — Gain moyen, risque faible

### P3.1 — Front : composant `SettingsCard` / usages de `FormLabel`
- [ ] Aligner `FormLabel` (`ui/Form.tsx:19`) sur le style settings (ou variante)
- [ ] Créer `SettingsCard` (Card + CardHeader icône/titre/description + CardContent)
- [ ] Créer `SettingsField` (label + `FormInput`/`FormSelect` avec save onBlur)
- [ ] Migrer les 45 labels inline : `SettingsPage.tsx`, `TelegramCard`, `QBittorrentCard`,
      `ProwlarrCard`, `KomgaSyncCard`, `AiTaggingCard`, `AnilistTab`, `TelegramMonitorCard`,
      `RenameFormatCard`
- [ ] Tests des cartes settings verts
- [ ] `npx tsc --noEmit` + `npm run lint` verts

### P3.2 — Front : composant `Switch`
- [ ] Créer `ui/Switch.tsx` (pattern `sr-only peer` réutilisable)
- [ ] Exporter depuis `ui/index.ts`
- [ ] Remplacer les toggles `TelegramCard.tsx:96,206`
- [ ] Rechercher d'autres toggles dupliqués et migrer
- [ ] Tests du Switch + `npx tsc --noEmit` verts

### P3.3 — Front : grille de stats pour les rapports de jobs
- [ ] Créer `ReportStatGrid` (grille de `StatBox` paramétrable)
- [ ] Migrer `MetadataReportCards`
- [ ] Migrer `ReadingStatusReportCards`
- [ ] Migrer `DownloadDetectionCards`
- [ ] Migrer `JobProgressCard`
- [ ] Tests des cartes de rapport verts

---

## P4 — Gain moyen, risque plus élevé

### P4.1 — Rust : factory de clients HTTP (31 occurrences)
- [ ] Créer `build_http_client(timeout)` / `HttpClientFactory` partagé
- [ ] Migrer `metadata_providers/*`
- [ ] Migrer `downloads/*`
- [ ] Migrer `integrations/*`
- [ ] Migrer `ai_tagging.rs`
- [ ] Migrer `crates/notifications/src/lib.rs:120`
- [ ] Homogénéiser timeouts et user-agent
- [ ] `cargo test --workspace` vert

### P4.2 — Rust : scraping `bdtheque` / `bdphile`
- [ ] Extraire `client()`, `text(el)`, `absolute(base, href)` dans un module de scraping commun
- [ ] Refactorer `bdtheque.rs`
- [ ] Refactorer `bdphile.rs`
- [ ] Tests des deux providers verts

### P4.3 — Rust : boilerplate trait `MetadataProvider`
- [ ] Évaluer macro / méthodes par défaut pour supprimer les 18 `Box::pin(async move {})`
- [ ] Implémenter l'approche retenue
- [ ] Migrer les 6 providers
- [ ] `cargo test --workspace` vert

### P4.4 — Découpage des gros fichiers
- [ ] `crates/parsers/src/lib.rs` (3301 l.) → `zip.rs`, `cbr.rs`, `pdf.rs`, `epub.rs`,
      `volume.rs`, `metadata.rs`
- [ ] `apps/api/src/downloads/telegram_monitor.rs` (2544 l.) → auth / sync canaux / jobs
- [ ] `apps/indexer/src/scanner.rs` (2283 l.) → discovery / mtime / classification volume_type
- [ ] `apps/api/src/stats.rs` (1571 l.) → agrégations par domaine
- [ ] `apps/backoffice/lib/api.ts` (2078 l.) → par domaine (books, series, metadata, downloads…)
- [ ] `cargo test --workspace` / `npm run test:unit` verts après chaque découpage

---

## Backlog — à évaluer plus tard

- [ ] Extraction de tous les helpers providers restants dans un module `text.rs` dédié
- [ ] Revue des autres `reqwest` non-builder / réutilisation de clients au sein d'un provider
- [ ] Audit global des autres usages `dangerouslySetInnerHTML` (hors settings)

---

## Journal de progression

| Lot | Statut | Date | Notes |
|-----|--------|------|-------|
| P0.1 | ☐ | | |
| P0.2 | ☐ | | |
| P0.3 | ☐ | | |
| P1.1 | ☐ | | |
| P1.2 | ☐ | | |
| P1.3 | ☐ | | |
| P1.4 | ☐ | | |
| P2.1 | ☐ | | |
| P2.2 | ☐ | | |
| P2.3 | ☐ | | |
| P3.1 | ☐ | | |
| P3.2 | ☐ | | |
| P3.3 | ☐ | | |
| P4.1 | ☐ | | |
| P4.2 | ☐ | | |
| P4.3 | ☐ | | |
| P4.4 | ☐ | | |
