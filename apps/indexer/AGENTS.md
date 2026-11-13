# apps/indexer — Service d'indexation

Service background sur le port **7081**. Voir `AGENTS.md` racine pour les conventions globales.

## Structure des fichiers

| Fichier | Rôle |
|---------|------|
| `main.rs` | Point d'entrée, initialisation, lancement du worker, démarrage du serveur HTTP |
| `lib.rs` | `AppState` (pool) |
| `worker.rs` | Boucle principale : claim job → process → cleanup stale ; lance watcher + scheduler ; boucle scheduler toutes les **60 s** |
| `job.rs` | `process_job` → `Result<JobOutcome, RunError>`, `finish` (statut terminal via `classify_job`), `JobCancelled`, `cleanup_stale_jobs` ; claim, échec et annulation viennent directement de `stripstream_jobs` |
| `scanner.rs` | Phase 1 discovery : WalkDir + `parse_metadata_fast` (zéro I/O archive), dossiers inchangés via mtime ; `scanner/reconcile.rs` (décision pure sur un fichier connu) et `scanner/writer.rs` (`ScanWriter`, seul écrivain) |
| `analyzer.rs` | Phase 2 analysis : claim `analysis_status`, ouvre chaque archive une fois (`analyze_book`), page_count + thumbnail (2 sous-phases, format configurable) |
| `converter.rs` | Conversion CBR → CBZ **in-process** via `parsers::convert_cbr_to_cbz` (crate `unrar`, aucun binaire externe) |
| `batch.rs` | `ScanBatch::flush` avec UNNEST (`BookInsert/BookUpdate/BookCorrection/FileInsert/FileUpdate/EventInsert`) ; les transitions d'analyse sont dans `stripstream_core::analysis` |
| `scheduler.rs` | 7 planifications : auto-scan, metadata refresh, reading-status push, download detection, Prowlarr RSS, Telegram sync, recherche DDL |
| `watcher.rs` | Polling filesystem toutes les **30 s** (snapshot, pas d'inotify) — crée des jobs `rebuild` |
| `api.rs` | Handlers HTTP de l'indexer (`/health`, `/version`, `/ready`) |
| `utils.rs` | `compute_fingerprint`, `kind_from_format`, `file_display_name` |

## Cycle de vie d'un job

```
stripstream_jobs::claim_next (SELECT ... FOR UPDATE SKIP LOCKED puis UPDATE, status pending→running)
  └─ process_job → JobOutcome { attempted, errors, stats } | RunError::{Cancelled, Failed}
       (le worker écrit ensuite job::finish ⇒ success / partial / failed, ou fail_job)
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

- **Verrouillage exclusif** : `claim_next` n'accorde un job exclusif (`rebuild`, `full_rebuild`,
  `rescan`, `scan`, `thumbnail_rebuild`, `thumbnail_regenerate`) que s'il n'y a **aucun** job actif
  (`running`, `extracting_pages`, `generating_thumbnails`). `cbr_to_cbz` n'est **pas** exclusif.
  Les types `API_ONLY_JOB_TYPES` (metadata_*, reading_status_push, rating_pull, download_detection,
  prowlarr_rss, telegram_sync, telegram_sync_incremental, ddl_search) ne sont jamais réclamés ici.
- **Annulation** : `is_job_cancelled` vérifié ~toutes les 10 fichiers ou 1 s (scanner) ; poller séparé
  de 2 s côté analyzer — retourne l'erreur typée `job::JobCancelled`, reconnue par type
  (`RunError::Cancelled`), jamais par son message.
- **Résultat** : un scan compte une unité par bibliothèque ; une bibliothèque en échec incrémente
  `stats.errors` sans arrêter le job, qui finit `partial` (au moins une réussie) ou `failed` (toutes
  en échec, `error_opt` = « n/m unit(s) failed »). Conversion et miniatures sont une unité unique.
- **Jobs stale** : `cleanup_stale_jobs` marque `failed` les jobs actifs dont `started_at < NOW() - 5 min`,
  les `pending` de plus de 30 min, et purge les jobs terminaux de plus de 90 jours.

## Pattern batch (batch.rs)

Les écritures de la discovery passent par `ScanBatch` (UNNEST), via `scanner::writer::ScanWriter` :

```rust
// Fichier connu : décision pure, puis un seul écrivain (séries, batch, événements)
let reconciliation = reconcile(existing_file, &parsed, Observation::of(existing_file, stat));
writer.apply(reconciliation, &abs_path, &file_name, stats).await?;
// Nouveau fichier
writer.insert_new(parsed, stat, &abs_path, &file_name, stats).await?;
// Le writer flush à BATCH_SIZE ; flush explicite à l'annulation et en fin de scan
writer.flush().await?;
```

Ne jamais écrire `books`/`book_files` ligne à ligne depuis la boucle de scan.

Toutes les opérations du flush sont dans une seule transaction.

## Scan filesystem — architecture 2 phases

### Phase 1 : Discovery (`scanner.rs`)

Pipeline allégé — **zéro ouverture d'archive** :
1. Charger `directory_mtimes` depuis la DB
2. WalkDir : pour chaque dossier, comparer mtime filesystem vs mtime stocké ; dans un dossier inchangé, seuls les fichiers déjà connus sont relus (même chemin que le cas normal : stat, fingerprint, progression, annulation, `reconcile`)
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
- Thumbnail en **2 sous-phases** via `stripstream_thumbnails::ThumbnailStore` : A) `stage_raw` vers
  `{book_id}.raw` ; B) `publish_raw` (encodage + publication atomique vers `{book_id}.{ext}`).
  Downscale rapide (`img.thumbnail()`, pas de Lanczos3), format `webp` (défaut) / `jpeg` / `png` /
  `original`. Régénération et nettoyage des orphelins passent par `purge` / `purge_orphans`.
- Config : `ThumbnailSettings::load_or_default` (clé `'thumbnail'`) + `app_settings.limits`

#### États d'analyse

| État | Signification | Sélectionné par |
|------|---------------|-----------------|
| `discovered` | Indexé, archive jamais ouverte | analyse complète |
| `analyzing` | Claim d'un job en cours (ou interrompu) | analyse complète (reprise) |
| `cover_pending` | Nombre de pages connu, miniature à générer | miniatures seules |
| `complete` | Nombre de pages et miniature connus | — |
| `failed` | Dernière analyse sans nombre de pages | analyse complète (retry) |

Toutes ces transitions vivent dans `stripstream_core::analysis` (ADR 0025) : `claim(pool, ids,
AnalysisMode)`, `release`, `mark_failed`, `record_page_counts`, `record_covers` (indexer),
`record_cover` (rendu à la volée de l'API) et `reset_covers` (régénération). La discovery n'écrit
que `discovered` via `ScanBatch`. N'écrivez jamais `analysis_status` en SQL ailleurs.

Transitions : skip/annulation d'un livre `analyzing` → `discovered` (un `cover_pending` le reste) ; erreur d'extraction ou page count irrécupérable →
`failed` ; page count récupéré → `cover_pending` ou `complete` selon la miniature déjà présente ;
miniature écrite → `complete`. Un échec de redimensionnement laisse l'état courant (`analyzing` en
analyse complète, `cover_pending` en miniatures seules) pour que le prochain job reprenne le livre.
Si les miniatures sont désactivées, l'analyse complète extrait seulement le page count
(`page_count_only`) et laisse le livre en `cover_pending` ; aucun fichier n'est supprimé par une
regénération. `failed` n'est pas compté comme « en attente » dans les diagnostics : il est exposé
séparément (`failed_analysis`).

## Configuration

- **Env** : `INDEXER_LISTEN_ADDR` (défaut `0.0.0.0:7081`), `INDEXER_SCAN_INTERVAL_SECONDS` (5)
- **DB** : `app_settings.thumbnail` (seule source des réglages de miniatures, sur les défauts
  `enabled=true`, `webp`, 300×400, qualité 80, `/data/thumbnails` ; aucune variable `THUMBNAIL_*`),
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
