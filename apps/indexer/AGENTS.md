# apps/indexer — Service d'indexation

Service background sur le port **7081**. Voir `AGENTS.md` racine pour les conventions globales.

## Structure des fichiers

| Fichier | Rôle |
|---------|------|
| `main.rs` | Point d'entrée, initialisation, lancement du worker, démarrage du serveur HTTP |
| `lib.rs` | `AppState` (pool) |
| `worker.rs` | Boucle principale : claim job → process → cleanup stale ; lance watcher + scheduler ; boucle scheduler toutes les **60 s** |
| `job.rs` | `claim_next_job` (verrouillage exclusif), `process_job`, `fail_job`, `cleanup_stale_jobs`, `is_job_cancelled` |
| `scanner.rs` | Phase 1 discovery : WalkDir + `parse_metadata_fast` (zéro I/O archive), skip dossiers inchangés via mtime, batching DB |
| `analyzer.rs` | Phase 2 analysis : ouvre chaque archive une fois (`analyze_book`), page_count + thumbnail (2 sous-phases, format configurable) |
| `converter.rs` | Conversion CBR → CBZ **in-process** via `parsers::convert_cbr_to_cbz` (crate `unrar`, aucun binaire externe) |
| `batch.rs` | `flush_all_batches` avec UNNEST, structures `BookInsert/BookUpdate/FileInsert/FileUpdate/ErrorInsert/EventInsert` |
| `scheduler.rs` | 6 planifications : auto-scan, metadata refresh, reading-status push, download detection, Prowlarr RSS, Telegram sync |
| `watcher.rs` | Polling filesystem toutes les **30 s** (snapshot, pas d'inotify) — crée des jobs `rebuild` |
| `api.rs` | Handlers HTTP de l'indexer (`/health`, `/version`, `/ready`) |
| `utils.rs` | `remap_libraries_path`, `unmap_libraries_path`, `compute_fingerprint`, `kind_from_format`, `file_display_name` |

## Cycle de vie d'un job

```
claim_next_job (SELECT ... FOR UPDATE SKIP LOCKED puis UPDATE, status pending→running)
  └─ process_job
       ├─ Phase 1 : scanner::scan_library_discovery
       │    ├─ WalkDir + parse_metadata_fast (zéro I/O archive)
       │    ├─ skip dossiers via directory_mtimes (table DB)
       │    └─ INSERT books (page_count=NULL) → livres visibles immédiatement
       ├─ analyzer::cleanup_orphaned_thumbnails (full_rebuild uniquement)
       └─ Phase 2 : analyzer::analyze_library_books
            ├─ SELECT books WHERE page_count IS NULL
            ├─ parsers::analyze_book(path, format, pdf_render_scale) → (page_count, first_page_bytes)
            ├─ generate_thumbnail (downscale rapide + encode, format configurable — WebP par défaut)
            └─ UPDATE books SET page_count, thumbnail_path

Jobs spéciaux :
  thumbnail_rebuild    → analyze_library_books(thumbnail_only=true)
  thumbnail_regenerate → regenerate_thumbnails (clear + re-analyze)
  cbr_to_cbz           → converter.rs (in-process, non exclusif)
  rescan               → vide directory_mtimes (par bibliothèque ou global) puis re-scan
  full_rebuild         → supprime les données, ignore fingerprints + directory_mtimes
```

- **Verrouillage exclusif** : `claim_next_job` n'accorde un job exclusif (`rebuild`, `full_rebuild`,
  `rescan`, `scan`, `thumbnail_rebuild`, `thumbnail_regenerate`) que s'il n'y a **aucun** job actif
  (`running`, `extracting_pages`, `generating_thumbnails`). `cbr_to_cbz` n'est **pas** exclusif.
  Les types `API_ONLY_JOB_TYPES` (metadata_*, reading_status_push, rating_pull, download_detection,
  prowlarr_rss, telegram_sync) ne sont jamais réclamés ici.
- **Annulation** : `is_job_cancelled` vérifié ~toutes les 10 fichiers ou 1 s (scanner) ; poller séparé
  de 2 s côté analyzer — retourne `Err("Job cancelled by user")`.
- **Jobs stale** : `cleanup_stale_jobs` marque `failed` les jobs actifs dont `started_at < NOW() - 5 min`,
  les `pending` de plus de 30 min, et purge les jobs terminaux de plus de 90 jours.

## Pattern batch (batch.rs)

Toutes les opérations DB massives passent par `flush_all_batches` avec UNNEST :

```rust
// Accumuler dans des Vec<BookInsert>, Vec<FileInsert>, etc.
books_to_insert.push(BookInsert { ... });

// Flush quand plein ou en fin de scan
if books_to_insert.len() >= BATCH_SIZE {
    flush_all_batches(&pool, &mut books_update, &mut files_update,
                      &mut books_insert, &mut files_insert,
                      &mut errors_insert, &mut events_insert).await?;
}
```

Toutes les opérations du flush sont dans une seule transaction.

## Scan filesystem — architecture 2 phases

### Phase 1 : Discovery (`scanner.rs`)

Pipeline allégé — **zéro ouverture d'archive** :
1. Charger `directory_mtimes` depuis la DB
2. WalkDir : pour chaque dossier, comparer mtime filesystem vs mtime stocké → skip si inchangé
3. Pour chaque fichier : `parse_metadata_fast` (title/series/volume depuis filename uniquement)
4. INSERT/UPDATE avec `page_count = NULL` — les livres sont visibles immédiatement
5. Upsert `directory_mtimes` en fin de scan

Fingerprint = SHA256(taille + mtime + filename) pour détecter les changements sans relire le fichier.

### Phase 2 : Analysis (`analyzer.rs`)

Traitement progressif en background :
- Query `WHERE page_count IS NULL` (ou `thumbnail_path IS NULL` pour thumbnail jobs)
- Concurrence bornée via `buffer_unordered(concurrency)` — défaut `(num_cpus / 2).clamp(1, 2)` soit
  1–2, surchargeable par `app_settings.limits.concurrent_renders`
- Par livre : `parsers::analyze_book(path, format, pdf_render_scale)` → `(page_count, first_page_bytes)`
- Thumbnail en **2 sous-phases** : A) extraction brute vers `{book_id}.raw` ; B) redimensionnement /
  encodage vers `{book_id}.{ext}`. Downscale rapide (`img.thumbnail()`, pas de Lanczos3), format
  `webp` (défaut) / `jpeg` / `png` / `original`
- UPDATE `books SET page_count, thumbnail_path`
- Config DB lue depuis `app_settings` (clés `'thumbnail'` et `'limits'`)

## Configuration

- **Env** : `INDEXER_LISTEN_ADDR` (défaut `0.0.0.0:7081`), `INDEXER_SCAN_INTERVAL_SECONDS` (5),
  `THUMBNAIL_ENABLED` (true), `THUMBNAIL_DIRECTORY` (`/data/thumbnails`), `THUMBNAIL_WIDTH` (300),
  `THUMBNAIL_HEIGHT` (400), `THUMBNAIL_QUALITY` (80), `THUMBNAIL_FORMAT` (webp)
- **Overrides DB** (prioritaires sur l'env) : `app_settings` → `thumbnail`,
  `limits.concurrent_renders`, `limits.timeout_seconds`
- **Logs** : `RUST_LOG` — défaut `indexer=info,axum=info,scan=info,extraction=info,thumbnail=warn,watcher=info`

## Path remapping

```rust
// abs_path en DB = chemin conteneur (/libraries/...)
// Sur l'hôte : LIBRARIES_ROOT_PATH remplace /libraries
utils::remap_libraries_path(&abs_path)    // DB → filesystem local
utils::unmap_libraries_path(&local_path) // filesystem local → DB
```

## Gotchas

- **Thumbnails** : générés **directement par l'indexer** (phase 2, `analyzer.rs`). L'API ne gère plus
  la génération — elle crée juste les jobs en DB.
- **page_count = NULL** : après la phase discovery, tous les nouveaux livres ont `page_count = NULL`.
  La phase analysis les remplit progressivement. Ne pas confondre avec une erreur.
- **directory_mtimes** : table DB qui stocke le mtime de chaque dossier scanné. Vidée au full_rebuild
  (et par le job `rescan`), mise à jour après chaque scan. Permet de skipper les dossiers inchangés.
- **full_rebuild** : supprime toutes les données puis re-insère. Ignore les fingerprints et les
  directory_mtimes.
- **Annulation** : vérifier `is_job_cancelled` régulièrement pour respecter les annulations utilisateur.
- **Watcher** : ce n'est **pas** un watcher temps réel — c'est un polling de snapshot toutes les 30 s
  (pas de crate `notify`/inotify). Il crée des jobs `rebuild` en cas de changement. Les suppressions
  déjà appliquées en base (livre/série supprimés via l'API) sont détectées et ignorées
  (`is_already_reconciled`) : pas de rebuild redondant. Seuls les ajouts et les suppressions externes
  (fichier toujours présent dans `book_files`) déclenchent un rebuild.
- **Watcher + scheduler** : tournent en tâches tokio séparées dans `worker.rs`, en parallèle de la
  boucle principale. Le scheduler vérifie 6 familles de jobs toutes les 60 s.
- **spawn_blocking** : l'ouverture d'archive (`analyze_book`) et la génération de thumbnail sont des
  opérations bloquantes — toujours les wrapper dans `tokio::task::spawn_blocking`.
- **Aucun outil externe** : la conversion CBR→CBZ et l'extraction passent par le crate `unrar` et
  `pdfium-render`, jamais par un binaire système.
