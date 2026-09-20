# Known Issues — dette technique & défauts potentiels

Ce document recense les défauts potentiels et incohérences de comportement relevés
lors des audits de couverture de tests de l'API (Rust) en septembre 2026.

Les comportements encore ouverts sont volontairement *verrouillés* par des tests
marqués `// LOCKED:` afin qu'un changement de comportement fasse échouer la suite
au lieu de passer inaperçu. Les défauts corrigés sont listés en §5.

> Convention : lorsqu'un point ci-dessous est corrigé, retirer le commentaire
> `// LOCKED:` du/des test(s) concerné(s), adapter les assertions, puis déplacer
> l'entrée vers §5.

---

## 1. Comportements suspects verrouillés par des tests

### `apps/api/src/libraries.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|

Mapping d'erreur concerné : `apps/api/src/error.rs` (`From<sqlx::Error>`, code PG 23505 → `ApiError::conflict`).

### `apps/api/src/metadata_providers/anilist.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 22 | Chapitres comptés comme tomes | Quand `volumes` est null, `total_volumes` retombe sur le nombre de **chapitres** (Berserk → 376 « tomes »). | `metadata_providers::anilist::tests::wiremock_fetch_trending_parses_results` |
| 23 | Livres synthétiques depuis les chapitres | `get_series_books` génère N livres « Vol. » à partir du nombre de chapitres (5 chapitres → 5 tomes). | `metadata_providers::anilist::tests::wiremock_get_series_books_chapters_fallback` |

### `apps/api/src/metadata_providers/google_books.rs`, `open_library.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 24 | `total_volumes` = nombre de résultats | Le total est le nombre de docs groupés par titre (plafonné à ~20) → une série de 40 tomes est annoncée à 20. | `metadata_providers::google_books::tests::search_series_parses_candidates`, `metadata_providers::open_library::tests::search_series_parses_candidates` |

### `apps/api/src/integrations/discovery/mod.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 25 | `external_id` écrasé sur doublon | Deux entités provider de même titre fusionnent en une série ; le lien est `ON CONFLICT DO UPDATE` → l'`external_id` d'origine est perdu. | `integrations::discovery::tests::test_add_to_library_duplicate_series` |
| 26 | Allowlist de liens codée en dur | Seul `senscritique` crée un `external_metadata_links` ; les autres providers ajoutés via discovery n'en créent aucun → sync metadata impossible. | `integrations::discovery::tests::test_add_to_library_anilist_no_metadata_link` |

### `apps/api/src/metadata_providers/bdtheque.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|

### `apps/api/src/metadata_providers/bdphile.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 36 | Erreurs de fetch album avalées | `fetch_album(...).unwrap_or_default()` masque les erreurs : un album en 500 produit quand même un livre, avec `authors`/`isbn`/`cover_url` vides. | `metadata_providers::bdphile::tests::album_fetch_error_is_swallowed` |

### `apps/api/src/metadata_providers/comicvine.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|

### `apps/api/src/tests/search.rs`

| # | Problème | Comportement actuel | Test qui verrouille |
|---|----------|---------------------|---------------------|
| 37 | SQL dupliqué dans les tests | Défaut de **test** (pas de prod) : `SERIES_SQL`/`BOOKS_SQL` recopient le SQL du handler au lieu de l'appeler → divergence silencieuse possible. | `tests::search::*` (constantes annotées `// LOCKED:`) |

Tous les candidats relevés lors de l'audit sont désormais verrouillés par un test
`// LOCKED:`.


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

**Décision assumée** : les tests ne sont pas exécutés en CI, faute de ressources
suffisantes sur le runner. La suite de tests doit être lancée **localement**
avant tout commit (`cargo test -p api`, `cargo clippy -p api --tests`).

---

## 4. Comment ces éléments sont maintenus

- Chaque comportement listé en §1 est couvert par un test annoté `// LOCKED:` ;
  modifier le comportement casse le test (signal explicite).
- Corriger un défaut = retirer l'annotation `// LOCKED:` + ajuster l'assertion,
  puis déplacer l'entrée vers §5.

---

## 5. Défauts corrigés

| # | Défaut | Correction | Test |
|---|--------|------------|------|
| 16 | `sanitize_filename` paniquait sur une frontière UTF-8 | Troncature sur frontière de caractère (`chars().take(200)`) au lieu d'un slice d'octets. | `books::rename::tests::sanitize_truncates_on_char_boundary` |
| 19 | Schéma `Bearer` sensible à la casse | Comparaison insensible à la casse (`eq_ignore_ascii_case`) du schéma d'authentification. | `users::auth::tests::bearer_token_lowercase_scheme_is_accepted` |
| 20 | `is_field_locked` échouait en mode ouvert | Les valeurs `"true"` (insensible à la casse) et les nombres non nuls sont désormais traités comme verrouillés. | `metadata::handlers::tests::field_locked_when_string_true`, `field_locked_when_nonzero_number` |
| 27 | Date invalide → `ended` | Une date non parseable renvoie désormais `"unknown"` au lieu du statut terminal `"ended"`. | `metadata_providers::senscritique::tests::infer_status_invalid_date_is_unknown` |
| 33 | Apostrophe → token parasite | `normalize_title` supprime l'apostrophe (`"JoJo's"` → `"jojos"`) au lieu de la remplacer par un espace. | `reading::status_match::tests::normalize_replaces_special_chars` |
| 30 | Pagination absente | `get_series_books_impl` itère sur `/ajax/series/tomes/{id}/{page}` jusqu'à une page vide. | `metadata_providers::bdtheque::tests::parses_series_and_volume_metadata` |
| 35 | `authors.sort()` réordonne les rôles | Le tri alphabétique est retiré : l'ordre scénariste/dessinateur de la source est conservé. | `metadata_providers::bdtheque::tests::parses_series_and_volume_metadata` |
| 31 | `external_book_id` = URL complète | Un id stable est extrait de l'URL d'album (`bd/138208-les-geants-1-erin`). | `metadata_providers::bdphile::tests::parses_series_and_album_details` |
| 32 | Métadonnées d'issue vides | `authors` est désormais rempli depuis `person_credits` ; `isbn`/`page_count` restent absents (non exposés par l'API issues). | `metadata_providers::comicvine::tests::get_series_books_parses_issues` |
| 1 | Doublon de `root_path` → 400 | Pré-check d'unicité dans `create_library` → **409 Conflict** ; le mapping sqlx 23505 renvoie aussi `conflict`. | `libraries::tests::create_library_rejects_duplicate_root_path` |
| 2 | `full` + `rescan` silencieux | La combinaison est rejetée en **400** (`full and rescan are mutually exclusive`) au lieu de préférer `full`. | `libraries::tests::scan_library_rejects_full_and_rescan_together` |
| 3 | `watcher_enabled` omis → `false` | Sémantique PATCH : `None` conserve la valeur stockée (`COALESCE($5, watcher_enabled)`). | `libraries::tests::update_monitoring_preserves_watcher_when_omitted` |
| 4 | `thumbnail_book_ids` sans `DISTINCT ON` | `update_monitoring` / `update_metadata_provider` appliquent le même `DISTINCT ON` (série) que `list_libraries`. | `libraries::tests::update_monitoring_returns_refreshed_counts_with_distinct_series` |
| 5 | `create_library` renvoyait un DTO synthétique | La réponse relit la ligne insérée (`RETURNING`) au lieu de la construire en dur. | `libraries::tests::create_library_trims_name_and_applies_defaults` |
| 6 | Description impossible à effacer | `description: Option<Option<String>>` distingue champ absent (conserve) de `null` explicite (efface). | `reading_lists::tests::update_reading_list_clears_description_on_explicit_null` |
| 7 | `reorder_series` ignorait les ids inconnus | Un id absent de la liste est rejeté en **400** au lieu d'être sauté silencieusement. | `reading_lists::tests::reorder_series_rejects_unknown_ids` |
| 8 | `by_format` mélangeait `kind` et `format` | Les livres sans `book_files` sont comptés sous `unknown` au lieu de `kind`. | `stats::tests::get_stats_by_format_uses_latest_file_then_kind_fallback` |
| 9 | `jobs_over_time` : `ELSE 'metadata'` | `prowlarr_rss` → `downloads`, `metadata_refresh_all` → `metadata`, tout le reste → `other`. | `stats::tests::get_stats_jobs_over_time_categorizes_job_types` |
| 10 | `users_reading_over_time` non scopé | Filtré sur l'utilisateur authentifié (`WHERE ($1::uuid IS NULL OR u.id = $1)`). | `stats::tests::get_stats_scopes_reading_to_authenticated_user` |
| 11 | Période invalide → `month` silencieux | Une `period` hors `{day, week, month}` est rejetée en **400**. | `stats::tests::get_stats_period_shapes_and_rejects_invalid` |
| 12 | `get_setting` documentait 404 mais renvoyait 200/null | Une clé absente renvoie désormais **404**, aligné sur l'annotation OpenAPI. | `settings::tests::get_setting_returns_not_found_for_unknown_key` |
| 13 | `rename_genre` créait un doublon | Le remplacement déduplique en préservant l'ordre d'origine (`WITH ORDINALITY`). | `genres::tests::rename_genre_merge_deduplicates` |
| 14 | Noms d'auteur blancs conservés | Le filtre utilise `btrim(author_name) <> ''`. | `authors::tests::list_authors_excludes_whitespace_only_names` |
| 15 | `total` = 0 hors plage | Le total est calculé par une requête dédiée, indépendante de la pagination. | `authors::tests::list_authors_out_of_range_page_returns_empty_with_total` |
| 17 | `failed_download_count` comptait des volumes | `COUNT(DISTINCT td.id)` compte les téléchargements, pas les volumes (`unnest`). | `downloads::detection::tests::failed_download_count_counts_downloads_not_volumes` |
| 18 | Une intégrale masquait tous les tomes manquants | Une intégrale numérotée ne couvre que son tome ; une intégrale sans numéro couvre toute la série. | `series::tests::missing_count_partial_integral_does_not_complete_series` |
| 21 | Sync purement additive | `classify_field_change` traite `new_value = None` comme un effacement quand `old_value` est présent. | `metadata::handlers::tests::classify_returns_cleared_when_new_is_none_and_old_is_some` |
