# apps/indexer — Service d'indexation

Service background sur le port **7081**. Voir `AGENTS.md` racine pour les conventions globales.

## Structure des fichiers

| Fichier | Rôle |
|---------|------|
| `main.rs` | Point d'entrée, initialisation, lancement du worker |
| `lib.rs` | `AppState` (pool) |
| `worker.rs` | Boucle principale : claim job → process → cleanup stale |
| `job.rs` | `claim_next_job`, `process_job`, `fail_job`, `cleanup_stale_jobs` |
| `scanner.rs` | Phase 1 discovery : WalkDir + `parse_metadata_fast` (zéro I/O archive), skip dossiers inchangés via mtime, batching DB |
| `analyzer.rs` | Phase 2 analysis : ouvre chaque archive une fois (`analyze_book`), génère page_count + thumbnail WebP |
| `batch.rs` | `flush_all_batches` avec UNNEST, structures `BookInsert/Update/FileInsert/Update/ErrorInsert` |
| `scheduler.rs` | Auto-scan : vérifie toutes les 60s les bibliothèques à monitorer |
| `watcher.rs` | File watcher temps réel |
| `api.rs` | Endpoints HTTP de l'indexer (/health, /ready) |
| `utils.rs` | `remap_libraries_path`, `unmap_libraries_path`, `compute_fingerprint`, `kind_from_format` |

## Cycle de vie d'un job

```
claim_next_job (UPDATE ... RETURNING, status pending→running)
  └─ process_job
       ├─ Phase 1 : scanner::scan_library_discovery
       │    ├─ WalkDir + parse_metadata_fast (zéro I/O archive)
       │    ├─ skip dossiers via directory_mtimes (table DB)
       │    └─ INSERT books (page_count=NULL) → livres visibles immédiatement
       ├─ analyzer::cleanup_orphaned_thumbnails (full_rebuild uniquement)
       └─ Phase 2 : analyzer::analyze_library_books
            ├─ SELECT books WHERE page_count IS NULL
            ├─ parsers::analyze_book → (page_count, first_page_bytes)
            ├─ generate_thumbnail (WebP, Lanczos3)
            └─ UPDATE books SET page_count, thumbnail_path

Jobs spéciaux :
  thumbnail_rebuild   → analyze_library_books(thumbnail_only=true)
  thumbnail_regenerate → regenerate_thumbnails (clear + re-analyze)
```

- Annulation : `is_job_cancelled` vérifié toutes les 10 fichiers ou 1s — retourne `Err("Job cancelled")`
- Jobs stale (running au redémarrage) → nettoyés par `cleanup_stale_jobs` au boot

## Pattern batch (batch.rs)

Toutes les opérations DB massives passent par `flush_all_batches` avec UNNEST :

```rust
// Accumuler dans des Vec<BookInsert>, Vec<FileInsert>, etc.
books_to_insert.push(BookInsert { ... });

// Flush quand plein ou en fin de scan
if books_to_insert.len() >= BATCH_SIZE {
    flush_all_batches(&pool, &mut books_update, &mut files_update,
                      &mut books_insert, &mut files_insert, &mut errors_insert).await?;
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
- Concurrence bornée (`futures::stream::for_each_concurrent`, défaut 4)
- Par livre : `parsers::analyze_book(path, format)` → `(page_count, first_page_bytes)`
- Génération thumbnail : resize Lanczos3 + encode WebP
- UPDATE `books SET page_count, thumbnail_path`
- Config lue depuis `app_settings` (clés `'thumbnail'` et `'limits'`)

## Path remapping

```rust
// abs_path en DB = chemin conteneur (/libraries/...)
// Sur l'hôte : LIBRARIES_ROOT_PATH remplace /libraries
utils::remap_libraries_path(&abs_path)    // DB → filesystem local
utils::unmap_libraries_path(&local_path) // filesystem local → DB
```

## Gotchas

- **Thumbnails** : générés **directement par l'indexer** (phase 2, `analyzer.rs`). L'API ne gère plus la génération — elle crée juste les jobs en DB.
- **page_count = NULL** : après la phase discovery, tous les nouveaux livres ont `page_count = NULL`. La phase analysis les remplit progressivement. Ne pas confondre avec une erreur.
- **directory_mtimes** : table DB qui stocke le mtime de chaque dossier scanné. Vidée au full_rebuild, mise à jour après chaque scan. Permet de skipper les dossiers inchangés en scan incrémental.
- **full_rebuild** : supprime toutes les données puis re-insère. Ignore les fingerprints et les directory_mtimes.
- **Annulation** : vérifier `is_job_cancelled` régulièrement pour respecter les annulations utilisateur.
- **Watcher + scheduler** : tournent en tâches tokio séparées dans `worker.rs`, en parallèle de la boucle principale.
- **spawn_blocking** : l'ouverture d'archive (`analyze_book`) et la génération de thumbnail sont des opérations bloquantes — toujours les wrapper dans `tokio::task::spawn_blocking`.
