# Un résultat de job typé dans l'indexer

L'indexer reconnaissait une annulation en cherchant « cancelled » dans le message d'erreur : toute erreur contenant ce mot devenait une annulation. Un scan dont une bibliothèque échouait incrémentait `stats.errors` mais finissait toujours `success`, alors que l'ADR 0002 prévoit `success`, `partial` ou `failed` via `classify_job`.

Désormais, le scanner et l'analyzer lèvent l'erreur typée `job::JobCancelled`. `process_job` renvoie `Result<JobOutcome { attempted, errors, stats }, RunError>`, avec `RunError::{Cancelled, Failed}` : l'annulation est reconnue par type (y compris sous un `context`), jamais par son message. Le worker écrit ensuite le statut terminal : `job::finish` (`classify_job` puis `finish_job` de `crates/jobs`) pour un résultat, `fail_job` pour un échec, et rien pour une annulation. Un scan compte une unité par bibliothèque. La conversion et les miniatures comptent une seule unité.

## Conséquences

- Un scan dont une partie des bibliothèques échoue finit `partial`. S'il échoue sur toutes, il finit `failed` avec `error_opt` = « n/m unit(s) failed » et déclenche la notification d'échec.
- `finish_job` accepte `Option<Value>` (`None` conserve `stats_json`) et vide `current_file`. `finish_success` est supprimé et le converter ne termine plus son job lui-même.
- Les relais `claim_next_job`, `fail_job` et `is_job_cancelled` de `job.rs` sont supprimés : les appelants utilisent `stripstream_jobs` directement.
