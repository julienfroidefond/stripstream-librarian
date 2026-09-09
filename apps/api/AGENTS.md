# apps/api — REST API (axum)

Service HTTP sur le port **7080**. Voir `AGENTS.md` racine pour les conventions globales.

## Structure des fichiers

| Fichier | Rôle |
|---------|------|
| `main.rs` | Routes, initialisation AppState, Semaphore concurrent_renders |
| `state.rs` | `AppState` (pool, caches, métriques), `load_concurrent_renders` |
| `auth.rs` | Middlewares `require_admin` / `require_read`, authentification tokens |
| `error.rs` | `ApiError` avec constructeurs `bad_request`, `not_found`, `internal`, etc. |
| `books.rs` | CRUD livres, thumbnails |
| `pages.rs` | Rendu page + double cache (mémoire LRU + disque) |
| `libraries.rs` | CRUD bibliothèques, déclenchement scans |
| `index_jobs.rs` | Suivi jobs, SSE streaming progression |
| `thumbnails.rs` | Rebuild/regénération thumbnails |
| `tokens.rs` | Gestion tokens API (create/revoke) |
| `settings.rs` | Paramètres applicatifs (stockés en DB, clé `limits`) |
| `openapi.rs` | Doc OpenAPI via utoipa, accessible sur `/swagger-ui` |

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
- **Tokens DB** : format `stl_<prefix>_<secret>`, hash argon2 en DB, scope `admin` ou `read`
- Middleware `require_admin` → routes admin ; `require_read` → routes lecture

### OpenAPI (utoipa)
```rust
#[utoipa::path(get, path = "/books/{id}", ...)]
async fn get_book(...) { }
// Ajouter le handler dans openapi.rs (ApiDoc)
```

### Cache pages (`pages.rs`)
- **Cache mémoire** : LRU limité en octets (`cache.memory_max_size_mb`, 128 Mo par défaut), redimensionné à chaud
- **Cache disque** : `IMAGE_CACHE_DIR` (défaut `/tmp/stripstream-image-cache`), clé SHA256
- Concurrence limitée par `AppState.page_render_limit` (Semaphore, configurable en DB)
- `spawn_blocking` pour le rendu image (CPU-bound)

### Paramètre concurrent_renders
Stocké en DB : `SELECT value FROM app_settings WHERE key = 'limits'` → JSON `{"concurrent_renders": N}`.
Chargé au démarrage dans `load_concurrent_renders`.

## Gotchas

- **LIBRARIES_ROOT_PATH** : les `abs_path` en DB commencent par `/libraries/`. Appeler `remap_libraries_path()` avant tout accès fichier.
- **Rate limit lecture** : middleware `read_rate_limit` sur les routes read (100 req/5s par défaut).
- **Métriques** : `/metrics` expose `requests_total`, `page_cache_hits`, `page_cache_misses` (atomics dans `AppState.metrics`).
- **Swagger** : accessible sur `/swagger-ui`, spec JSON sur `/openapi.json`.
