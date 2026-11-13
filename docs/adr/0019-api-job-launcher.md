# Un seul lanceur de jobs d'arrière-plan dans l'API

Les handlers qui lancent un job exécuté dans le processus de l'API (lot et rafraîchissement de
métadonnées, correspondance, envoi et récupération des statuts de lecture, détection de
téléchargements, flux RSS) recopiaient tous la même séquence : boucle sur les bibliothèques,
`job_in_flight`, `INSERT index_jobs … 'running'`, nom de bibliothèque, `tokio::spawn`, `fail_job`,
puis notification. Les copies avaient divergé.

`apps/api/src/jobs/launcher.rs` porte désormais cette séquence. Chaque type est décrit par un
`JobSpec` (type, types en conflit, notification d'échec, statistiques partielles), défini dans le
module qui planifie le job, conformément à l'ADR 0002. `launch` lance un job sur une bibliothèque ou
un job global (`library_id IS NULL`) ; `launch_each` lance un job par bibliothèque. L'échec passe
par `report_failure` : statistiques partielles, `fail_job` de `crates/jobs` (l'annulation reste
prioritaire), puis notification. Le poller de jobs réutilise ce chemin pour les jobs qu'il réclame.
Les endpoints renvoient `JobLaunchResponse { id, status }`, dont `status` vaut `running`,
`already_running` ou `started` (lancement sur toutes les bibliothèques ; `id` est alors le dernier job
créé ou `null`).

## Conséquences

- Un lancement sur toutes les bibliothèques vérifie les mêmes conflits qu'un lancement sur une
  seule : un lot global ne démarre plus à côté d'un `metadata_batch_rematch`. Il respecte aussi
  `force_rematch`, que le mode global ignorait.
- Le décompte des liens à rafraîchir n'existe plus qu'une fois et une erreur SQL n'est plus
  confondue avec « aucun lien ». La vérification des liens précède désormais celle des conflits.
- Un lancement qui crée un seul job répond `running` partout, y compris les métadonnées qui
  répondaient `pending` alors que le job était déjà `running`, et le RSS global qui répondait
  `started`. Le JSON garde la forme
  `{ id, status }` lue par le backoffice, qui n'utilise que `id`.
- `prowlarr_rss` lancé à la main notifie son échec comme quand il est réclamé par le poller, et
  `reading_status_push` réclamé par le poller conserve ses statistiques partielles comme quand il est
  lancé à la main.
- Les jobs mis en file `pending` pour le poller ou l'indexeur (scan, miniatures, Telegram, DDL) ne
  passent pas par le lanceur.
