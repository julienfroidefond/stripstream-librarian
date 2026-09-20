# Known Issues — dette technique & défauts potentiels

Ce document recense les défauts potentiels et incohérences de comportement relevés
lors des audits de couverture de tests de l'API (Rust) en septembre 2026.

**Rien n'est corrigé ici.** Les comportements concernés sont volontairement
*verrouillés* par des tests marqués `// LOCKED:` afin qu'un changement de
comportement fasse échouer la suite au lieu de passer inaperçu.

> Convention : lorsqu'un point ci-dessous est corrigé, retirer le commentaire
> `// LOCKED:` du/des test(s) concerné(s) et adapter les assertions.

---

## 1. Comportements suspects verrouillés par des tests

### `apps/api/src/libraries.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 1 | Doublon de `root_path` | La contrainte `UNIQUE` remonte en **HTTP 400** (`bad_request`) via le mapping sqlx 23505, et non 409 Conflict. Aucun pré-check d'unicité dans le handler. | `libraries::tests::create_library_rejects_duplicate_root_path` |
| 2 | `scan_library` avec `full` + `rescan` | `full` gagne silencieusement → type de job `full_rebuild`, le deep rescan est perdu. | `libraries::tests::scan_library_full_takes_precedence_over_rescan` |
| 3 | `update_monitoring` et `watcher_enabled` omis | Traité comme `false` → le watcher est **désactivé** alors que le client ne l'a pas demandé. | `libraries::tests::update_monitoring_enables_schedule_and_defaults_watcher_off` |
| 4 | `thumbnail_book_ids` incohérent | `list_libraries` applique `DISTINCT ON` (série) + `LIMIT 5`, mais `update_monitoring` / `update_metadata_provider` utilisent un `LIMIT 5` simple → peuvent renvoyer plusieurs tomes de la **même** série. | `libraries::tests::update_monitoring_returns_refreshed_counts_without_distinct_series` (et `list_libraries_thumbnail_ids_pick_first_book_per_series`) |
| 5 | `create_library` renvoie un DTO synthétique | La réponse est construite en dur (défauts `monitor_enabled=false`, `scan_mode="manual"`, compteurs 0) au lieu de relire la ligne insérée → peut mentir si les défauts DB changent. | `libraries::tests::create_library_trims_name_and_applies_defaults` |

Mapping d'erreur concerné : `apps/api/src/error.rs` (`From<sqlx::Error>`, code PG 23505 → `ApiError::bad_request`).

### `apps/api/src/reading_lists/`

| # | Problème | Cause | Test qui verrouille |
|---|----------|-------|---------------------|
| 6 | Impossible d'effacer une description | `description: Option<String>` confond « champ absent » et `null` ; le handler ne met à jour que si `is_some()`. Aucune sentinelle pour « clear ». | `reading_lists::tests::update_reading_list_cannot_clear_description` |
| 7 | `reorder_series` ignore les ids inconnus | Les ids absents sont silencieusement sautés, les autres gardent l'index du payload → trous dans `position`. Aucune erreur renvoyée. | `reading_lists::tests::reorder_series_ignores_unknown_ids` |

### `apps/api/src/stats.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 8 | `by_format` mélange `kind` et `format` | `COALESCE(bf.format, b.kind)` : les livres sans `book_files` sont comptés sous `comic` / `ebook` / `bd`, qui ne sont pas des formats de fichier → le breakdown de formats mélange deux nomenclatures. | `stats::tests::get_stats_by_format_uses_latest_file_then_kind_fallback` |
| 9 | `jobs_over_time` : bucket `ELSE 'metadata'` | Tout `index_jobs.type` absent du `CASE` (`prowlarr_rss`, `torrent_import`, `rating_pull`, `telegram_sync`, `telegram_sync_incremental`) est silencieusement compté comme `metadata`. | `stats::tests::get_stats_jobs_over_time_categorizes_job_types` |
| 10 | `users_reading_over_time` non scopé | Sur `/stats`, `reading_status`, `currently_reading`, `recently_read` et `reading_over_time` sont filtrés sur l'utilisateur authentifié, mais `users_reading_over_time` (`CROSS JOIN users`) renvoie les lectures de **tous** les utilisateurs. | `stats::tests::get_stats_users_reading_over_time_is_not_scoped_to_user` |
| 11 | Période invalide → `month` silencieux | Une valeur `period` inconnue (`yearly`, …) tombe dans la branche `_` et repart sur une granularité mensuelle, sans erreur ni indication. | `stats::tests::get_stats_period_shapes_and_invalid_falls_back_to_month` |

### `apps/api/src/settings.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 12 | `get_setting` documente 404 mais renvoie 200/null | L'annotation OpenAPI du handler déclare `(status = 404, description = "Setting not found")`, mais une clé absente répond **200** avec `null` — l'erreur 404 n'est jamais produite. | `settings::tests::get_setting_returns_null_for_unknown_key` |

### `apps/api/src/genres.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 13 | `rename_genre` peut créer un doublon | `array_replace` remplace l'ancien nom par le nouveau sans vérifier sa présence : renommer `Aventure` en `Action` sur une série qui a déjà `Action` produit `{Action, Action}` au lieu de fusionner. | `genres::tests::rename_genre_merge_creates_duplicate` |

### `apps/api/src/authors.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 14 | Noms d'auteur blancs conservés | Le filtre SQL n'exclut que `NULL` et `''` (`author_name <> ''`), pas les chaînes d'espaces : un auteur `"  "` apparaît comme une entrée à part entière. | `authors::tests::list_authors_keeps_whitespace_only_names` |
| 15 | `total` = 0 hors plage | Le total fenêtré (`COUNT(*) OVER()`) est lu sur la première ligne renvoyée ; une page hors plage ne renvoie aucune ligne → `total` vaut **0** alors que des auteurs existent. | `authors::tests::list_authors_out_of_range_page_returns_empty_with_total` |

### `apps/api/src/books/rename.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 16 | `sanitize_filename` panique sur UTF-8 | La troncature utilise `trimmed.len()` (octets) puis `trimmed[..200]` (slice d'octets) : un caractère multi-octets à cheval sur l'octet 200 provoque un **panic**. Le test ASCII existant masque le bug. | `books::rename::tests::sanitize_panics_on_multibyte_boundary` |

### `apps/api/src/downloads/detection.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 17 | `failed_download_count` compte des volumes | La requête de `get_latest_found` fait `COUNT(*)` sur `unnest(td.expected_volumes)` → le champ compte les **volumes** échoués, pas les téléchargements. Le test historique utilise une requête sans `unnest` et valide donc une requête obsolète. | `downloads::detection::tests::failed_download_count_counts_volumes_not_downloads` |

### `apps/api/src/series/helpers.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 18 | Une intégrale masque tous les tomes manquants | `COUNT(...) FILTER (WHERE volume_type='integral') > 0 THEN 0` : une intégrale partielle (1 tome sur 10) marque la série **complète**. `get_missing_books` met en plus `total_local = total_external` et vide `missing_books`. | `series::tests::missing_count_zero_when_integral_plus_regular` |

### `apps/api/src/users/auth.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 19 | Schéma `Bearer` sensible à la casse | `strip_prefix("Bearer ")` rejette `bearer xxx` en 401. RFC 7235 §2.1 définit le schéma comme **insensible à la casse**. | `users::auth::tests::bearer_token_lowercase_scheme_is_rejected` |

### `apps/api/src/metadata/handlers.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 20 | `is_field_locked` échoue en mode ouvert | Une valeur `locked_fields` non booléenne (`"true"`, `1`) est traitée comme **non verrouillée** → la sync provider peut écraser une édition manuelle. | `metadata::handlers::tests::field_not_locked_when_string_true` |

### Candidats relevés mais non verrouillés

Comportements suspects identifiés lors de l'audit des tests pré-existants, non
encore couverts par un test `// LOCKED:` dédié :

| Zone | Comportement suspect |
|------|----------------------|
| `metadata_providers/anilist.rs` | `total_volumes` = nombre de **chapitres** quand `volumes` est null (Berserk → 376 « tomes ») ; `get_series_books` génère N livres synthétiques depuis les chapitres. |
| `metadata_providers/google_books.rs`, `open_library.rs` | `total_volumes` = nombre de résultats de recherche (plafonné à ~20) → une série de 40 tomes est annoncée à 20. |
| `integrations/discovery/mod.rs` | Deux entités provider distinctes de même titre fusionnent en une série ; le lien existant est `ON CONFLICT DO UPDATE` → l'`external_id` d'origine est perdu. |
| `integrations/discovery/mod.rs` | Allowlist de liens codée en dur (`bedetheque`/`senscritique`) : les autres providers ajoutés via discovery ne créent aucun `external_metadata_links`. |
| `metadata_providers/senscritique.rs` | `infer_status_from_date` : date invalide → `"ended"` (statut terminal) au lieu d'`"unknown"`. |
| `metadata_providers/bedetheque.rs` | Page 200 vide classée « rate-limited » ; cover URL fabriquée même quand l'enrichissement 404 ; covers associées par index positionnel. |
| `metadata_providers/bdtheque.rs` | `authors.sort()` réordonne scénariste/dessinateur ; pagination absente (`/tomes/{id}/0` seulement). |
| `metadata_providers/bdphile.rs` | Erreurs de fetch album avalées (`unwrap_or_default()`) ; `external_book_id` = URL complète. |
| `metadata_providers/comicvine.rs` | `authors`/`isbn`/`page_count` toujours vides pour les issues. |
| `reading/status_match.rs` | `normalize_title` : `"JoJo's"` → `"jojo s"` (apostrophe → token parasite). |
| `metadata/handlers.rs` | `classify_field_change` / `diff_opt_str` : `new=None` = « pas de changement » → sync purement additive, jamais de suppression. |
| `apps/api/src/tests/search.rs` | Défaut de **test** (pas de prod) : le SQL du handler est recopié en constantes (`SERIES_SQL`/`BOOKS_SQL`) au lieu d'appeler le handler → divergence silencieuse possible. |


---

## 2. Couverture de tests — modules API encore sans tests

État au 2026-09-20 (après couverture de `libraries.rs`, `reading_lists.rs`, `stats.rs`, `settings.rs`, `genres.rs`, `ai_tagging.rs`, `authors.rs`, `handlers.rs` et `api_middleware.rs`, puis audit des tests pré-existants).
Les sous-modules (`downloads/`, `metadata/`, `metadata_providers/`, `series/`,
`reading/`, `integrations/`, `books/`, `jobs/`, `users/`, …) disposent déjà de tests.

| Module | LOC (fichier) | Priorité |
|--------|---------------|----------|
| — | — | — |

Tous les modules de premier niveau listés lors de l'audit sont désormais couverts.

Aucun test HTTP/router de bout en bout n'existe : les tests appellent les handlers
directement avec un `AppState` construit à la main.

---

## 3. Intégration continue

`.gitea/workflows/deploy.yml` ne lance **ni `cargo test` ni `cargo clippy`**.
Les garanties apportées par la suite de tests et les lints ne sont donc pas
vérifiées avant publication.

---

## 4. Comment ces éléments sont maintenus

- Chaque comportement listé en §1 est couvert par un test annoté `// LOCKED:` ;
  modifier le comportement casse le test (signal explicite).
- Corriger un défaut = retirer l'annotation `// LOCKED:` + ajuster l'assertion,
  puis mettre à jour ce document.
