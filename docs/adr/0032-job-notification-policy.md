# La politique des notifications d'un job vit dans `notifications`

Le worker de l'indexer décidait lui-même quels messages Telegram un job produisait. Il
chargeait `index_jobs`, `libraries`, `books` et les événements `book_added` avant d'appeler
`notifications::notify` cinq fois : annulation, échec, une notification par série découverte,
puis un résumé conditionnel (`series.is_empty() || removed_files > 0 || errors > 0`). Le module
`notifications` ne voyait donc qu'un `NotificationEvent` déjà construit : il possédait le
formatage et les interrupteurs, pas la connaissance du job.

Deux schémas de `stats_json` coexistaient pour la même valeur :

| Écrivain | Lecteur |
|---|---|
| `scanner::JobStats` — 8 champs, `serde::Serialize` | `notifications::ScanStats` — 7 champs, sans serde, parsé champ par champ dans le worker |

`warnings` (walk partiel, chemins non UTF-8, sous-arbres sautés) existait côté écrivain et était
perdu à la lecture : le backoffice l'affichait, Telegram jamais.

`crates/notifications/src/job.rs` porte désormais la politique. `notify_job(pool, JobFinished)`
est le seul point d'entrée : `JobFinished` dit *ce qui est arrivé* (`job_id`, `library_id`,
`JobOutcome::{Completed, Failed, Cancelled}`, durée mesurée par le worker), et le module charge
tout le reste. `job_events(pool, &JobFinished)` rend la lecture en base et la décision testables
sans Telegram ; `events_for(&JobFinished, JobFacts)` est la politique pure, sans I/O ; `deliver`
garde l'interrupteur, le filtre de pertinence, le formatage et l'envoi, partagés avec `notify`
(que l'API continue d'utiliser).

La règle du résumé est écrite à un seul endroit, avec sa raison :

- une série découverte parle par son message de série ;
- le résumé est émis pour un run silencieux (aucune série) ou bruyant (suppressions, erreurs) ;
- `is_noteworthy` continue d'écarter un run où rien ne s'est passé.

## Décisions

- **Un seul schéma de stats** : `stripstream_jobs::ScanStats` (serde, `warnings` inclus) est le
  format de `index_jobs.stats_json`. `scanner::JobStats` en est un alias, le backoffice garde les
  mêmes noms de champs, et un ancien JSON sans `warnings` décode en `0` via `#[serde(default)]`.
- **La politique est côté lecteur** : un nouveau type de job n'ajoute pas de branche dans
  `worker.rs` ; il choisit sa catégorie dans `job_type_category` et son événement dans
  `events_for`. Le worker ne connaît plus que `JobFinished`.
- **`notify_job` charge la configuration une fois** et envoie les événements d'un job
  séquentiellement, dans l'ordre produit (séries puis résumé). `notify` reste disponible pour les
  appels ponctuels de l'API.
- **Les avertissements apparaissent dans le résumé** (`🔶 Warnings`) sans le rendre
  « noteworthy » à eux seuls : un scan partiel qui n'a rien indexé reste silencieux, comme avant.
- **Périmètre** : les jobs de l'indexer. Les jobs pilotés par l'API (miniatures, régénération de
  couvertures) continuent d'appeler `notify` depuis leurs handlers ; les y brancher demanderait
  de leur fournir un `JobFinished`, pas de déplacer la politique.

## Conséquences

- Le worker ne lit plus `stats_json` ni les événements `book_added` : il signale la fin du job.
  `cleanup_available_downloads` reste dans l'indexer (c'est une écriture du domaine indexation).
- Une divergence de comportement entre deux jobs est désormais visible dans `events_for`, pas
  répartie entre le worker et le module.
- Les tests du module couvrent les deux coutures : `events_for` par variante d'issue et de
  catégorie, `job_events` contre une vraie base (`#[sqlx::test]`), y compris la conservation des
  `warnings` et le regroupement des livres d'une série.
- Le message Telegram d'un scan gagne la ligne `Warnings` ; tous les autres messages sont
  inchangés.
