# apps/indexer — Service d'indexation

Service background sur le port **7081**. Voir `AGENTS.md` racine pour les conventions globales.

## Structure des fichiers

| Fichier | Rôle |
|---------|------|
| `main.rs` | Point d'entrée, initialisation, lancement du worker |
| `lib.rs` | `AppState` (pool, meili, api_base_url) |
| `worker.rs` | Boucle principale : claim job → process → cleanup stale |
| `job.rs` | `claim_next_job`, `process_job`, `fail_job`, `cleanup_stale_jobs` |
| `scanner.rs` | Scan filesystem, parsing parallèle (rayon), batching DB |
| `batch.rs` | `flush_all_batches` avec UNNEST, structures `BookInsert/Update/FileInsert/Update/ErrorInsert` |
| `scheduler.rs` | Auto-scan : vérifie toutes les 60s les bibliothèques à monitorer |
| `watcher.rs` | File watcher temps réel |
| `meili.rs` | Indexation/sync Meilisearch |
| `api.rs` | Appels HTTP vers l'API (pour checkup thumbnails) |
| `utils.rs` | `remap_libraries_path`, `unmap_libraries_path`, `compute_fingerprint`, `kind_from_format` |

## Cycle de vie d'un job

```
claim_next_job (UPDATE ... RETURNING, status pending→running)
  └─ process_job
       ├─ scanner::scan_library  (rayon par_iter pour le parsing)
       │    └─ flush_all_batches toutes les BATCH_SIZE=100 itérations
       └─ meili sync
            └─ api checkup thumbnails (POST /index/jobs/:id/thumbnails/checkup)
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

## Scan filesystem (scanner.rs)

Pipeline en 3 étapes :
1. **Collect** : WalkDir → filtrer par format (CBZ/CBR/PDF)
2. **Parse** : `file_infos.into_par_iter().map(parse_metadata)` (rayon)
3. **Process** : séquentiel pour les inserts/updates DB

Fingerprint = SHA256(taille + mtime) pour détecter les changements sans relire le fichier.

## Path remapping

```rust
// abs_path en DB = chemin conteneur (/libraries/...)
// Sur l'hôte : LIBRARIES_ROOT_PATH remplace /libraries
utils::remap_libraries_path(&abs_path)    // DB → filesystem local
utils::unmap_libraries_path(&local_path) // filesystem local → DB
```

## Gotchas

- **Thumbnails** : générés par l'API après handoff, pas par l'indexer directement. L'indexer appelle `/index/jobs/:id/thumbnails/checkup` via `api.rs`.
- **full_rebuild** : si `true`, ignore les fingerprints → tous les fichiers sont retraités.
- **Annulation** : vérifier `is_job_cancelled` régulièrement pour respecter les annulations utilisateur.
- **Watcher + scheduler** : tournent en tâches tokio séparées dans `worker.rs`, en parallèle de la boucle principale.
