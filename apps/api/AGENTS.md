# apps/api — REST API (axum)

Service HTTP sur le port **7080**. Voir `AGENTS.md` racine pour les conventions globales.

## Structure des fichiers

| Chemin | Rôle |
|--------|------|
| `main.rs` | Routes, initialisation `AppState`, Swagger UI (client + admin) |
| `state.rs` | `AppState` (pool, caches, métriques), `page_render_limit` (Semaphore), `load_concurrent_renders` |
| `coverage.rs` | Adaptateurs DB de `stripstream_core::coverage::SeriesCoverage` : CTE canonique des manquants, chargement par lot/unité, volumes possédés par nom de série |
| `api_middleware.rs` | Middlewares `require_admin` / `require_read`, rate limiting lecture |
| `error.rs` | `ApiError` avec constructeurs `bad_request`, `not_found`, `internal`, etc. |
| `handlers.rs` | `/health`, `/ready`, `/version`, `/metrics` |
| `openapi.rs` | `ClientApiDoc` + `AdminApiDoc` (dual spec via utoipa) |
| `books/` | `mod.rs` (CRUD livres), `pages.rs` (rendu + double cache), `thumbnails.rs` (jobs), `rename.rs` |
| `metadata/` | `shared_sync.rs`, `batch.rs`/`batch_sync.rs`, `refresh.rs`/`refresh_sync.rs`, `handlers.rs`, `config.rs`, `sync.rs` |
| `metadata_providers/` | `mod.rs` (trait + `available_providers`), `google_books`, `open_library`, `comicvine`, `anilist`, `bdtheque`, `bdphile`, `senscritique` |
| `integrations/` | `anilist*`, `komga`, `telegram`, `discovery/` |
| `downloads/` | `prowlarr`, `qbittorrent`, `torrent_import`, `import_pipeline`, `detection`, `missing`, `telegram_monitor`, `rss_poll` |
| `jobs/` | `index_jobs.rs` (suivi + SSE), `poller.rs`, `helpers.rs` |
| `reading/` | `progress.rs`, `status_match.rs`, `status_pull.rs`, `status_push.rs` |
| `series/` | `list`, `create`, `merge`, `update`, `archive`, `favorites`, `ratings`, `recommendations`, `related`, `ongoing`, `helpers` |
| `reading_lists/` | Listes de lecture |
| `users/` | `accounts.rs`, `auth.rs`, `tokens.rs`, `genre_restrictions.rs` |
| `libraries.rs`, `search.rs`, `settings.rs`, `stats.rs`, `authors.rs`, `genres.rs`, `responses.rs`, `ai_tagging.rs` | Endpoints transverses |
| `diagnostics/` | `mod.rs` (`GET /diagnostics` : agrégat versions, services, base, disques, caches, miniatures, jobs, intégrité, bibliothèques) + `database.rs`, `storage.rs`, `jobs.rs`, `integrity.rs`, `runtime.rs`, `probe.rs` (`GET /diagnostics/probe` : intégrations externes + fichiers manquants, **lancé à la demande**). Lecture seule, aucune donnée sensible ; dégrade en 200 avec `database.reachable = false` plutôt qu'en 500. |
| `tests/` | Tests unitaires + `#[sqlx::test]` (DB temporaire migrée) |

## Patterns clés

### Handler type
```rust
async fn my_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MyDto>, ApiError> {
    // ...
}
```

### Erreurs API
```rust
// Constructeurs disponibles dans error.rs
ApiError::bad_request("message")
ApiError::not_found("resource not found")
ApiError::internal("unexpected error")
ApiError::unauthorized("missing token")
ApiError::forbidden("admin required")

// Conversion auto depuis sqlx::Error et std::io::Error
```

### Authentification
- **Bootstrap token** : comparaison directe (`API_BOOTSTRAP_TOKEN`), scope Admin
- **Tokens DB** : format `stl_<prefix>_<secret>`, hash argon2 en DB, scope `admin` ou `read` (`users/tokens.rs`)
- Middlewares dans `api_middleware.rs` : `require_admin` → routes admin ; `require_read` → routes lecture

### OpenAPI (utoipa)
```rust
#[utoipa::path(get, path = "/books/{id}", ...)]
async fn get_book(...) { }
// Ajouter le handler dans openapi.rs (ClientApiDoc / AdminApiDoc)
```
- Client : `/openapi.json` + UI `/swagger-ui` — Admin : `/admin/openapi.json` + UI `/admin/swagger-ui`

### Cache pages (`books/pages.rs`)
- **Cache mémoire** : LRU limité en octets (`cache.memory_max_size_mb`, 128 Mo par défaut), redimensionné à chaud
- **Cache disque** : `IMAGE_CACHE_DIR` (défaut `/tmp/stripstream-image-cache`), clé SHA256
- Concurrence limitée par `AppState.page_render_limit` (Semaphore, configurable en DB)
- `spawn_blocking` pour le rendu image (CPU-bound)

### Paramètre concurrent_renders
Stocké en DB : `SELECT value FROM app_settings WHERE key = 'limits'` → JSON `{"concurrent_renders": N}`.
Chargé au démarrage dans `state.rs::load_concurrent_renders`.

## Gotchas

- **Diagnostic / heartbeats** : l'API et l'indexeur écrivent `service_heartbeats` toutes les 30 s
  (`stripstream_core::heartbeat::spawn`, seuil hors-ligne 90 s). Déployer les deux services ensemble :
  sinon la carte Services signale un badge *Versions différentes*.
- **LIBRARIES_ROOT_PATH** : les `abs_path` en DB commencent par `/libraries/`. Passer par
  `AppState.library_root.resolve(&db_path)` puis `to_local()` avant tout accès fichier ; ne jamais
  concaténer `/libraries` à la main.
- **Rate limit lecture** : middleware `read_rate_limit` sur les routes read, fenêtre de 1s **par client** (clé = token `Authorization`), limite `rate_limit_per_second` (défaut 120). Dépassement → `429` JSON `{"error":"rate limit exceeded"}` + `Retry-After: 1`.
- **Métriques** : `/metrics` expose `requests_total`, `page_cache_hits`, `page_cache_misses` (atomics dans `AppState.metrics`).
- **Thumbnails** : la génération en masse est faite par l'indexer (phase 2), mais `GET
  /books/{id}/thumbnail` peut rendre et persister une miniature à la demande. Cette persistance ne
  promeut que `cover_pending` → `complete` ; elle n'écrase jamais `analyzing` ou `failed`.
- **Swagger** : accessible sur `/swagger-ui`, spec JSON sur `/openapi.json`.
- **Torrent import, mode replace** : quand `replace_existing = true`, ne PAS filtrer par
  `expected_volumes` — importer tous les fichiers du torrent.
- **Téléchargements torrent** : `torrent_downloads` est un ledger partagé par qBittorrent et
  AllDebrid. Toutes ses écritures passent par `downloads/torrent_ledger.rs` : `TorrentStatus`
  porte le vocabulaire des statuts et les transitions gardées (`mark_completed`, `begin_import`,
  `finish_import`, `fail`, `restart`) refusent un statut inattendu. Ne pas écrire
  `UPDATE torrent_downloads` ailleurs ; la progression (`record_progress`, `arm_stall`) est un
  no-op sur une ligne qui n'est plus `downloading`.
- **Compteurs d'import** : `ImportedFile.already_existed`
  (`downloads/torrent_import.rs`) distingue les fichiers réellement copiés de ceux déjà sur
  disque. Les compteurs et les notifications n'incluent que les vrais nouveaux fichiers.
- **Détection de torrent en double** : quand un torrent existant est trouvé par hash de magnet,
  vérifier que son `content_path` existe sur disque avant de lancer l'import. Les anciens
  répertoires `sl-*` sont nettoyés après l'import.
- **Disponibilités** : `stripstream_core::availability` possède la fusion
  (`merge_releases`), le prune (`prune_series` / `prune_library`) et la normalisation
  (`normalize_library`) des releases `available_downloads`. Le `missing_count` ne décrémente que
  des volumes distincts réellement retirés d'au moins une release — jamais du nombre de fichiers
  importés. Ne pas refiltrer le JSON dans les handlers.
- **Identité de série** : `stripstream_core::series_identity` possède la clé normalisée
  (`norm_text`, casse + accents) et les lookups par nom brut (unitaire, batch, multi-bibliothèques).
  Pour rattacher un nom de torrent/détection/discovery à une série, passer par ce module ; ne pas
  écrire de nouveau `LOWER(name)` ni de nouveau join `norm_text` à la main.
- **Livres manquants** : `external_book_metadata` peut contenir plusieurs éditions pour un même
  `volume_number`. Les requêtes d'affichage utilisent `DISTINCT ON (volume_number)`.
