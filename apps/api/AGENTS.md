# apps/api — REST API (axum)

Service HTTP sur le port **7080**. Voir `AGENTS.md` racine pour les conventions globales.

## Structure des fichiers

| Chemin | Rôle |
|--------|------|
| `main.rs` | Routes, initialisation `AppState`, Swagger UI (client + admin) |
| `state.rs` | `AppState` (pool, caches, métriques), `page_render_limit` (`RenderLimit`, redimensionnable à chaud) |
| `settings.rs`, `settings/limits.rs` | Endpoints `/settings`, `apply` (unique point de rechargement à chaud) ; schéma typé `Limits` de `app_settings.limits` (ADR 0026) |
| `coverage.rs` | Adaptateurs DB de `stripstream_core::coverage::SeriesCoverage` : CTE canonique des manquants, chargement par lot/unité, volumes possédés par nom de série |
| `api_middleware.rs` | Middlewares `require_admin` / `require_read`, rate limiting lecture |
| `error.rs` | `ApiError` avec constructeurs `bad_request`, `not_found`, `internal`, etc. |
| `handlers.rs` | `/health`, `/ready`, `/version`, `/metrics` |
| `openapi.rs` | `ClientApiDoc` + `AdminApiDoc` (dual spec via utoipa) |
| `books/` | `mod.rs` (CRUD livres), `pages.rs` (handlers page + miniature à la volée), `renderer.rs` (`PageRenderer` : double cache, verrou, permis, timeout), `files.rs` (fichier courant d'un livre), `download.rs`, `thumbnails.rs` (jobs), `rename.rs` |
| `metadata/` | `shared_sync.rs`, `batch.rs`/`batch_sync.rs`, `refresh.rs`/`refresh_sync.rs`, `handlers.rs`, `config.rs`, `sync.rs` |
| `metadata_providers/` | `mod.rs` (trait + `available_providers`), `scoring.rs` (score de correspondance + verdict d'auto-match), `google_books`, `open_library`, `comicvine`, `anilist`, `bdtheque`, `bdphile`, `senscritique` |
| `integrations/` | `anilist*`, `komga`, `telegram`, `discovery/` |
| `downloads/` | `prowlarr`, `qbittorrent`, `torrent_import`, `import_pipeline`, `detection`, `missing`, `telegram_monitor`, `rss_poll` |
| `jobs/` | `index_jobs.rs` (suivi + SSE), `launcher.rs` (lancement des jobs in-process), `poller.rs`, `helpers.rs` |
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

### Rendu et cache des pages (`books/renderer.rs`, ADR 0023)
- `PageRenderer::get(&PageSpec, Urgency)` est le seul chemin de rendu : cache mémoire → cache disque → verrou par clé → permis → timeout → `spawn_blocking`. `Urgency::Prefetch` ne rend jamais une page déjà en cache ou en cours de rendu. `PageRenderer::run` (permis + timeout + `spawn_blocking`) sert au travail non caché (miniature à la volée).
- Archives derrière le trait `PageSource` (`ArchivePages` en prod, `FakePages` en test) : ne pas appeler `parsers::extract_page` depuis un handler.
- **Cache mémoire** : LRU limité en octets (`cache.memory_max_size_mb`, 128 Mo par défaut), redimensionné à chaud
- **Cache disque** : `IMAGE_CACHE_DIR` (défaut `/tmp/stripstream-image-cache`), clé SHA256 (chemin, mtime, page, options)
- Concurrence limitée par `AppState.page_render_limit` (`RenderLimit` : sémaphore redimensionné à chaud par `limits.concurrent_renders`)
- `spawn_blocking` pour le rendu image (CPU-bound)

### Fichier courant d'un livre (`books/files.rs`)
- `current_book_file(pool, &library_root, book_id)` → `BookFile { db_path, local_path, format: BookFormat }` (404 sans fichier). Le format vient toujours de la base, jamais de l'extension.
- Requêtes ensemblistes : `concat!(…, current_book_file_join!("size_bytes"), …)` (alias `b` pour `books`, `bf` pour le fichier). Ne pas réécrire `ORDER BY updated_at DESC LIMIT 1` sur `book_files`.

### Réglage `limits` (ADR 0026)
`settings/limits.rs` est le seul schéma de `app_settings.limits` : chaque champ y a un `Spec`
(défaut, min, max) et un type `NonZero*` dans `Limits`. `Limits::parse` valide à l'écriture
(`POST /settings/limits` → 400 listant chaque champ fautif) ; `Limits::load` / `from_stored` lit
la ligne (un entier stocké hors plage est borné, un champ illisible retombe sur son défaut). Ne pas relire le JSON `limits` à la
main ni re-clamper une valeur : dériver de `Limits` (`DownloadSettings::from(&limits)`,
`DynamicSettings::apply_limits`). Tout rechargement à chaud passe par `settings::apply(state, key)`,
y compris `concurrent_renders` (les rendus en cours ne sont jamais interrompus).

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
  /books/{id}/thumbnail` peut rendre et persister une miniature à la demande
  (`pages::render_book_cover` + `ThumbnailStore::publish`, mêmes format/dimensions/qualité/répertoire
  que l'indexer via `ThumbnailSettings::load`). Cette persistance ne promeut que `cover_pending` →
  `complete` ; elle n'écrase jamais `analyzing` ou `failed`. Le répertoire des miniatures (configuré
  et défaut) est protégé contre `cache.directory`.
- **Swagger** : accessible sur `/swagger-ui`, spec JSON sur `/openapi.json`.
- **Torrent import, mode replace** : quand `replace_existing = true`, ne PAS filtrer par
  `expected_volumes` — importer tous les fichiers du torrent.
- **Téléchargements torrent** : `torrent_downloads` est un ledger partagé par qBittorrent et
  AllDebrid. Toutes ses écritures passent par `downloads/torrent_ledger.rs` : `TorrentStatus`
  porte le vocabulaire des statuts et les transitions gardées (`mark_completed`, `begin_import`,
  `finish_import`, `fail`, `restart`) refusent un statut inattendu. Ne pas écrire
  `UPDATE torrent_downloads` ailleurs ; la progression (`record_progress`, `arm_stall`) est un
  no-op sur une ligne qui n'est plus `downloading`.
- **Progression de transfert** : `downloads/progress.rs` possède le contrat des colonnes
  (`bytes`, `progress` 0..=1, `download_speed`, `eta`) et la règle de taille : la colonne
  `file_size` n'est jamais effacée (`None` comme `0` conservent la valeur stockée, seule une
  valeur positive la remplace). Chaque flux compose `TransferRate` +
  `TransferProgress::{from_bytes, from_reported}` avec un `ProgressSink` (`TorrentSink`,
  `DdlSink`, `TelegramSink`) ; `Reporter` porte le throttle et le calcul d'ETA du moteur ranged.
  Ne pas réécrire de throttle, de formule d'ETA ni d'`UPDATE` de progression dans un flux.
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
- **Lancement de jobs in-process** : un handler qui crée un job exécuté par l'API le décrit par un
  `JobSpec` (type, conflits, notification d'échec, stats partielles) et passe par
  `jobs/launcher.rs` (`launch` pour une bibliothèque ou un job global, `launch_each` pour toutes).
  Ne pas réécrire `job_in_flight` + `INSERT index_jobs` + `tokio::spawn` + `fail_job` dans un
  handler ; renvoyer `JobLaunchResponse` (ADR 0019).
- **Identité de série** : `stripstream_core::series_identity` possède la clé normalisée
  (`norm_text`, casse + accents) et les lookups par nom brut (unitaire, batch, multi-bibliothèques).
  Pour rattacher un nom de torrent/détection/discovery à une série, passer par ce module ; ne pas
  écrire de nouveau `LOWER(name)` ni de nouveau join `norm_text` à la main. Pour trouver ou créer
  une série à partir d'un nom, `series_identity::find_or_create` (ADR 0022), jamais un
  `INSERT INTO series ... ON CONFLICT (library_id, name)`.
- **Liens de métadonnées** : `metadata/links.rs` est le seul écrivain de
  `external_metadata_links` (ADR 0017) : `propose`, `approve`, `approve_existing`,
  `set_primary`, `remove(Removal::…)`, `reassign`, `record_provider_snapshot`, `mark_synced`.
  Il garde l'invariant « un seul primaire approuvé par série » et applique une politique unique
  après retrait d'un lien approuvé (promotion, puis refusion des champs de série ou effacement).
  Ne pas écrire `INSERT`/`UPDATE`/`DELETE external_metadata_links` ailleurs.
- **Livres manquants** : `external_book_metadata` peut contenir plusieurs éditions pour un même
  `volume_number`. Les requêtes d'affichage utilisent `DISTINCT ON (volume_number)`.
