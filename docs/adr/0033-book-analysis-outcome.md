# L'analyse d'un livre a un verdict typé, écrit en base par un seul module

`apps/indexer/src/analyzer.rs` faisait deux choses à la fois : il orchestrait les deux
sous-phases du job (claim, lots, progression, statuts `extracting_pages` /
`generating_thumbnails`) et il décidait, pour chaque livre, ce qu'il fallait écrire. Le même
vocabulaire y était traduit deux fois, dans deux blocs de 120 lignes séparés par 250 lignes :

| Phase A | Phase B |
|---|---|
| `ExtractOutcome::{Error, EventOnly, PageCountOnly, Thumbnail, Skipped, ResourceFork}` | `PublishOutcome::{Published, PageCountOnly}` |
| `analysis::record_page_counts` + `flush_parse_errors` + `clear_parse_errors` + `mark_failed` + `release` | `analysis::record_covers` + `record_page_counts` |
| événement `pages_extracted` / `error`, texte inline | événement `thumbnail_generated` / `pages_extracted`, texte inline |

La chaîne `"Recovered {} pages, thumbnail skipped — {}"` existait en deux exemplaires, la
suppression des resource forks macOS était une paire de `DELETE` au milieu de la boucle, et
rien ne testait la traduction sinon le test d'intégration complet (un CBZ synthétique par
scénario).

`apps/indexer/src/analyzer.rs` devient le module `analyzer/` :

- `analyzer/mod.rs` — l'orchestration du job : sélection et claim, lots, concurrence bornée,
  progression, annulation, régénération et purge des orphelins. Rien d'autre.
- `analyzer/book.rs` — le pipeline d'**un** livre. `BookAnalyzer` porte les réglages (store,
  racine, timeout, échelle PDF, miniatures actives) ; `extract` rend un `Extracted`, `publish`
  rend un `Published`. Les deux rendent le **verdict** du livre, jamais une écriture SQL.
- `analyzer/record.rs` — la traduction verdict → base : `record_extraction` (page counts, parse
  errors, `failed`, release, resource forks, retour des pages mises de côté) et
  `record_publication` (couvertures, page counts, événement). Les textes d'événements y sont
  construits une seule fois (`pages_event` sert les deux phases).

Les deux sous-phases restent : leurs statuts de job, leur barre de progression 0-50 puis 50-100
et leurs lots de 200 sont visibles dans le backoffice, et `extracting_pages` suivi de
`generating_thumbnails` est ce qui distingue un job bloqué en I/O d'un job bloqué en CPU.

## Décisions

- **Un verdict par livre, par phase** : `Extracted` et `Published` sont des enums fermés. Un
  nouveau cas (par exemple un format non supporté à traiter autrement) s'ajoute au type et le
  compilateur pointe les deux traductions concernées.
- **La traduction est un module, pas une méthode du pipeline** : `record` ne connaît ni archive
  ni miniature, seulement des verdicts ; il peut donc être testé avec des verdicts construits à
  la main, sans ouvrir un seul fichier. `BookAnalyzer` ne connaît ni `analysis_status` ni la
  table des événements.
- **Les transitions d'état restent dans `stripstream_core::analysis`** (ADR 0025). `record`
  appelle `record_page_counts`, `record_covers`, `mark_failed`, `release` — il n'écrit aucun
  `analysis_status` en SQL, et `claim` reste dans l'orchestration.
- **Ordre des écritures inchangé** : resource forks, page counts, parse errors, nettoyage,
  `failed`, release, événements. Un lot qui échoue sur la suppression d'un resource fork
  interrompt toujours le job (`?`), comme avant.
- **Comportement visible inchangé** : mêmes textes d'événements (dont le `warning` de page
  count récupéré), mêmes transitions (`cover_pending` sans miniature, `complete` avec),
  mêmes logs, même progression. Un échec d'encodage en phase B enregistre toujours le page
  count et supprime le `.raw`.
- **`converter.rs` hors périmètre** : sa fusion de doublon CBR → CBZ suit un autre cycle de vie
  (transaction de `book_files` + progression de lecture) et n'a en commun avec l'analyse que
  deux `DELETE`. Elle garde son propre chemin.

## Conséquences

- `analyzer.rs` (950 lignes) devient un module d'orchestration de ~480 lignes et deux fichiers
  de ~330 et ~250 lignes, chacun avec une seule raison de changer.
- La chaîne d'événement « Recovered … » n'existe plus qu'à un endroit : une phase ne peut plus
  recevoir un texte que l'autre a corrigé.
- Tests : `record` est couvert par des tests unitaires des constructeurs d'événements et par
  des `#[sqlx::test]` par verdict (page count récupéré + statut `cover_pending`, échec +
  `parse_status`, resource fork supprimé, page mise de côté restant `analyzing`, couverture
  publiée → `complete`). Les tests d'intégration existants de `analyze_library_books`
  (page count + thumbnail, archive corrompue, régénération, orphelins) continuent de couvrir
  l'orchestration.
- `AGENTS.md` de l'indexer décrit les trois fichiers et la règle : un nouveau comportement par
  livre se décide dans `book.rs`, s'écrit dans `record.rs`, et ne se réimplémente pas dans
  `mod.rs`.
