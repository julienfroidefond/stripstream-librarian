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

---

## 2. Couverture de tests — modules API encore sans tests

État au 2026-09-19 (après couverture de `libraries.rs` et `reading_lists.rs`).
Les sous-modules (`downloads/`, `metadata/`, `metadata_providers/`, `series/`,
`reading/`, `integrations/`, `books/`, `jobs/`, `users/`, …) disposent déjà de tests.

| Module | LOC (fichier) | Priorité |
|--------|---------------|----------|
| `stats.rs` | ~1491 | 1 |
| `settings.rs` | ~522 | 2 |
| `genres.rs` | ~284 | 3 |
| `ai_tagging.rs` | ~284 | 3 |
| `authors.rs` | ~138 | 4 |
| `handlers.rs` | ~83 | 4 |
| `api_middleware.rs` | ~61 | 4 |

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
