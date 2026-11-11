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
| `analyzer.rs` | Phase 2 analysis : claim `analysis_status`, ouvre chaque archive une fois (`analyze_book`), page_count + thumbnail (2 sous-phases, format configurable) |
| `converter.rs` | Conversion CBR → CBZ **in-process** via `parsers::convert_cbr_to_cbz` (crate `unrar`, aucun binaire externe) |
| `batch.rs` | `flush_all_batches` avec UNNEST, structures `BookInsert/BookUpdate/FileInsert/FileUpdate/ErrorInsert/EventInsert`, `claim_analysis_books` / `flush_analysis_status` |
| `scheduler.rs` | 7 planifications : auto-scan, metadata refresh, reading-status push, download detection, Prowlarr RSS, Telegram sync, recherche DDL |
| `watcher.rs` | Polling filesystem toutes les **30 s** (snapshot, pas d'inotify) — crée des jobs `rebuild` |
| `api.rs` | Handlers HTTP de l'indexer (`/health`, `/version`, `/ready`) |
| `utils.rs` | `compute_fingerprint`, `kind_from_format`, `file_display_name` |

## Cycle de vie d'un job

```
claim_next_job (SELECT ... FOR UPDATE SKIP LOCKED puis UPDATE, status pending→running)
  └─ process_job
       ├─ Phase 1 : scanner::scan_library_discovery
       │    ├─ WalkDir + parse_metadata_fast (zéro I/O archive)
       │    ├─ skip dossiers via directory_mtimes (table DB)
       │    └─ INSERT books (analysis_status='discovered', page_count=NULL) → livres visibles immédiatement
       ├─ analyzer::cleanup_orphaned_thumbnails (full_rebuild uniquement)
       └─ Phase 2 : analyzer::analyze_library_books
            ├─ claim analysis_status='analyzing' (discovered / failed / analyzing)
            ├─ parsers::analyze_book(path, format, pdf_render_scale) → (page_count, first_page_bytes)
            ├─ generate_thumbnail (downscale rapide + encode, format configurable — WebP par défaut)
            └─ UPDATE books SET page_count, thumbnail_path, analysis_status

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
  prowlarr_rss, telegram_sync, telegram_sync_incremental, ddl_search) ne sont jamais réclamés ici.
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
4. INSERT/UPDATE avec `analysis_status = 'discovered'` et `page_count = NULL` — les livres sont visibles immédiatement
5. Upsert `directory_mtimes` en fin de scan

Fingerprint = SHA256(taille + mtime + filename) pour détecter les changements sans relire le fichier.

### Phase 2 : Analysis (`analyzer.rs`)

Traitement progressif en background, piloté par `books.analysis_status` (voir ci-dessous) :
- Analyse complète : claim `discovered`, `failed` et `analyzing` (reprise après job interrompu).
- Miniatures seules (`thumbnail_only`) : claim `cover_pending` uniquement.
- Concurrence bornée via `buffer_unordered(concurrency)` — défaut `(num_cpus / 2).clamp(1, 2)` soit
  1–2, surchargeable par `app_settings.limits.concurrent_renders`
- Par livre : `parsers::analyze_book(path, format, pdf_render_scale)` → `(page_count, first_page_bytes)`
- Si `analyze_book` échoue (rendu de la 1re page impossible, timeout…), `extract_page_count`
  retente via `parsers::page_count_only` : le livre conserve son `page_count` (donc reste visible)
  sans vignette, et un événement `pages_extracted` de niveau `warning` est enregistré.
- Thumbnail en **2 sous-phases** : A) extraction brute vers `{book_id}.raw` ; B) redimensionnement /
  encodage vers `{book_id}.{ext}`. Downscale rapide (`img.thumbnail()`, pas de Lanczos3), format
  `webp` (défaut) / `jpeg` / `png` / `original`
- Config DB lue depuis `app_settings` (clés `'thumbnail'` et `'limits'`)

#### États d'analyse

| État | Signification | Sélectionné par |
|------|---------------|-----------------|
| `discovered` | Indexé, archive jamais ouverte | analyse complète |
| `analyzing` | Claim d'un job en cours (ou interrompu) | analyse complète (reprise) |
| `cover_pending` | Nombre de pages connu, miniature à générer | miniatures seules |
| `complete` | Nombre de pages et miniature connus | — |
| `failed` | Dernière analyse sans nombre de pages | analyse complète (retry) |

Transitions : skip/annulation → `discovered` ; erreur d'extraction ou page count irrécupérable →
`failed` ; page count récupéré → `cover_pending` ou `complete` selon la miniature déjà présente ;
miniature écrite → `complete`. Un échec de redimensionnement laisse l'état courant (`analyzing` en
analyse complète, `cover_pending` en miniatures seules) pour que le prochain job reprenne le livre.
Si les miniatures sont désactivées, l'analyse complète extrait seulement le page count
(`page_count_only`) et laisse le livre en `cover_pending` ; aucun fichier n'est supprimé par une
regénération. `failed` n'est pas compté comme « en attente » dans les diagnostics : il est exposé
séparément (`failed_analysis`).

## Configuration

- **Env** : `INDEXER_LISTEN_ADDR` (défaut `0.0.0.0:7081`), `INDEXER_SCAN_INTERVAL_SECONDS` (5),
  `THUMBNAIL_ENABLED` (true), `THUMBNAIL_DIRECTORY` (`/data/thumbnails`), `THUMBNAIL_WIDTH` (300),
  `THUMBNAIL_HEIGHT` (400), `THUMBNAIL_QUALITY` (80), `THUMBNAIL_FORMAT` (webp)
- **Overrides DB** (prioritaires sur l'env) : `app_settings` → `thumbnail`,
  `limits.concurrent_renders`, `limits.timeout_seconds`
- **Logs** : `RUST_LOG` — défaut `indexer=info,axum=info,scan=info,extraction=info,thumbnail=warn,watcher=info`

## Path remapping

`AppState.library_root` est construit une seule fois depuis la config
(`LibraryRoot::from_env`, `crates/core/src/paths.rs`), puis passé aux fonctions qui touchent aux
fichiers. `LibraryRoot` expose `from_db_path`, `from_local_path` et `resolve` ; le résultat
`LibraryPath` porte `to_local()` et `to_db()`. Ne jamais joindre `/libraries` à la main.

```rust
let path = state.library_root.resolve(&db_path)?;
std::fs::read(path.to_local())
// chemin local → DB :
state.library_root.resolve(&local_path)?.to_db()
```

`resolve` traite dans l'ordre : préfixe DB `/libraries` → racine locale → chemin absolu inchangé
(bibliothèque hors montage, données historiques).

## Gotchas

- **Thumbnails** : générés **directement par l'indexer** (phase 2, `analyzer.rs`). L'API ne crée que
  les jobs en DB ; elle peut toutefois rendre et persister une miniature à la demande (`GET
  /books/{id}/thumbnail`), ce qui fait passer un livre `cover_pending` à `complete`.
- **analysis_status** : source de vérité de l'avancement de l'analyse. Ne pas déduire l'état de
  `page_count IS NULL` ou `thumbnail_path IS NULL` : un livre `failed` n'a pas de page count sans
  être en attente, et un livre `discovered` peut déjà avoir une miniature rendue par l'API.
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
  boucle principale. Le scheduler vérifie 7 familles de jobs toutes les 60 s.
- **spawn_blocking** : l'ouverture d'archive (`analyze_book`) et la génération de thumbnail sont des
  opérations bloquantes — toujours les wrapper dans `tokio::task::spawn_blocking`.
- **Propagation de `volume_type`** : sur les dossiers skippés (mtime inchangé) comme sur les
  fichiers dont le fingerprint n'a pas changé, le scanner re-compare et réécrit
  `books.volume_type`. Un simple rescan suffit donc à corriger des livres mal classés
  HS/oneshot/integral.
- **Nettoyage des disponibilités** : `cleanup_available_downloads` construit un
  `stripstream_core::coverage::SeriesCoverage` par série puis délègue le prune à
  `stripstream_core::availability::prune_library`. Une intégrale non numérotée supprime la ligne
  `available_downloads` ; une intégrale numérotée ne prune que son propre tome. HS, one-shots et
  volume `0` ne comptent jamais. Le `missing_count` ne décrémente que des volumes distincts
  réellement retirés d'au moins une release (un tome proposé par deux releases compte une fois).
- **Aucun outil externe** : la conversion CBR→CBZ et l'extraction passent par le crate `unrar` et
  `pdfium-render`, jamais par un binaire système.
