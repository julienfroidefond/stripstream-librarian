# Plan des chantiers d'architecture — novembre 2026

Plan de suivi des refactors issus de l'audit d'architecture du 2026-11-12. Le rapport d'origine
(snapshot HTML interactif, avec les schémas avant/après) est versionné à côté :
[`architecture-review-20261112-1948.html`](./architecture-review-20261112-1948.html).

L'audit se régénère avec la skill `improve-codebase-architecture` (elle écrit son rapport dans le
répertoire temporaire ; le snapshot ci-dessus est la copie de référence).

## Chantiers

| # | Zone | Titre | Force | Statut |
|---|------|-------|-------|--------|
| 01 | Downloads · API Rust | Faire du cycle de vie des torrents un ledger gardé | Strong | **fusionné** (#280) |
| 02 | Downloads · API Rust | Composer un module de progression de transfert unique | Strong | **fusionné** (#285) |
| 03 | Downloads · API Rust | Un résolveur de cible de bibliothèque validé | Strong | **fusionné** (#279) |
| 04 | Downloads · API Rust | Enregistrer les releases appariées une seule fois | Worth exploring | hors périmètre |
| 05 | Indexer · Rust | Réconcilier les corrections du scanner une seule fois | Worth exploring | repris en **13** (Strong) |
| 06 | Reading lists · API Rust | Supprimer la table d'accents locale | Worth exploring | hors périmètre |
| 07 | Downloads · Backoffice | Un seam d'écriture unique pour les téléchargements | Strong | **fusionné** (#286) |
| 08 | Series detail · Backoffice | Un loader de détail de série, un view model | Strong | **fusionné** (#283) |
| 09 | Jobs · Backoffice | Un descripteur de type de job et un seam SSE unique | Strong | **fusionné** (#284) |
| 10 | Reading lists · Backoffice | Une politique d'écriture optimiste unique | Worth exploring | réévalué en **22** |

Périmètre retenu : les six chantiers **Strong** (03, 01, 02, 07, 08, 09). Les quatre « Worth
exploring » ne sont pas planifiés.

## Audit du 2026-11-13 — zones non couvertes

Second passage (grille *deep modules* : module, interface, seam, test de suppression) sur ce que
l'audit du 12 n'avait pas couvert : API hors downloads, indexer, crates partagés, backoffice hors
downloads / série / jobs. Tous les chantiers sont retenus.

### Bugs trouvés pendant l'audit

| # | Zone | Bug | Traité par |
|---|------|-----|------------|
| B1 | Discovery · API Rust | La requête des tomes possédés lit `books.volume_number` (la colonne est `volume`) ; l'erreur est avalée, `volumes_already_owned` est toujours vide | PR #292 |
| B2 | Reading lists · Backoffice | `moveSeries` appelle `.../series/{id}/move`, route inexistante ; chaque moitié est mockée dans les tests | PR #289 |
| B3 | Settings · Backoffice | API injoignable ⇒ brouillon amorcé avec les défauts sans signal ; une sauvegarde peut écraser la config stockée | PR #291 (chantier 17) |
| B4 | Metadata · API Rust | Le bonus « nombre de tomes » s'applique après le plafond 0,7 : une variante plafonnée peut auto-matcher | PR #297 |
| B5 | Metadata · API Rust | La fusion de séries déplace les liens externes sans gérer `is_primary` (index unique « un primaire par série ») | PR #294 (chantier 12) |

### Chantiers

| # | Zone | Titre | Force | Statut |
|---|------|-------|-------|--------|
| 11 | Jobs · API Rust | Un seul lanceur de jobs d'arrière-plan | Strong | PR #296 |
| 12 | Metadata · API Rust | Un module propriétaire de `external_metadata_links` (corrige B5) | Strong | PR #294 |
| 13 | Indexer · Rust | Réconcilier un fichier existant une seule fois (ex-05) | Strong | PR #293 |
| 14 | Crates · Rust | Un magasin de miniatures partagé API / indexer | Strong | PR #295 |
| 15 | Crates · Rust | `series_identity::find_or_create` (prolonge l'ADR 0007) | Strong | à faire (après 12, 13) |
| 16 | Cache · Backoffice | Une seule politique d'invalidation du cache Next | Strong | PR #290 |
| 17 | Settings · Backoffice | Un loader pour `/settings` (corrige B3) | Strong | PR #291 |
| B4 | Metadata · API Rust | Une seule politique de score de correspondance | Strong | PR #297 |
| 18 | Books · API Rust | Un rendu de page unique et un `current_book_file` | Worth exploring | à faire |
| 19 | Indexer · Rust | Un résultat de job typé (annulation, `partial`) | Worth exploring | à faire (après 14) |
| 20 | Crates · Rust | Les transitions de `analysis_status` dans un seul module | Worth exploring | à faire (après 14) |
| 21 | Settings · API Rust | Un schéma typé pour `app_settings.limits` | Worth exploring | à faire |
| 22 | Reading lists · Backoffice | Un store d'écriture unique (ex-10) | Worth exploring | à faire |
| 23 | Genres · Backoffice + API | Une requête de navigation par genres côté serveur | Worth exploring | à faire |
| 24 | Libraries / Series · Backoffice | Passer le DTO plutôt que ses champs en props | Worth exploring | à faire |
| 25 | Workspace | Ménage : `core2-shim`, `MonitoringForm.tsx`, `BookFormat::from_str` | — | à faire |

Décisions notables de la vague 1 :

- **14** : la config des miniatures ne vient que de `app_settings.thumbnail` ; les variables `THUMBNAIL_*` (jamais lues) sont supprimées.
- **12** : une seule politique après retrait d'un lien approuvé (promotion puis refusion depuis les liens stockés, ou effacement) ; l'upsert de création de série garde volontairement l'identité existante (`OnConflict::KeepExisting`, #25).
- **B4** : bonus nombre de tomes appliqué avant le plafond ; SensCritique note par similarité. Reste ouvert : la variante « article déplacé + sous-titre retiré » de `scraping::query_variants` n'est pas plafonnée.
- **11** : le batch metadata global respecte désormais `force_rematch` et les mêmes conflits que le mode par bibliothèque.

Constats détaillés (avec références `fichier:ligne`) : voir les descriptions des PR et les ADR
0015 et suivantes.

Correction du snapshot du 12 pour le chantier 10 : seul le réordonnancement est optimiste ;
le vrai problème est l'existence de quatre politiques d'échec pour douze écritures.

Vagues d'exécution (pour limiter les conflits) : **vague 1** = B1, B2, B4, 11, 12, 13, 14, 16,
17 ; **vague 2** = 15, 18, 19, 20, 21, 22, 23, 24, 25, une fois la vague 1 fusionnée.

## Méthode

- Un chantier = une branche = une PR (squash merge), review avant fusion, jamais de merge directe
  sur `main`.
- Chaque chantier livre une ADR dans `docs/adr/` (format court : décision + conséquences).
- Tout changement de code livre ses tests. Vérifications avant commit :
  - Rust : `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
    `cargo nextest run --workspace` (ou `cargo test -p api`) ;
  - Backoffice : `npx tsc --noEmit`, `npm run lint`, `npm run test:unit`.
- Pas de bump de version ni de `CHANGELOG.md` manuel : le workflow de déploiement le fait sur
  `main`.
- `apps/docs` n'est mis à jour que si le comportement visible change ; les notes de module vont
  dans le `AGENTS.md` de l'app concernée.
- Exécution : un worktree par chantier, un sous-agent (agent `builder`) par chantier, en avant-plan ;
  les chantiers indépendants tournent en parallèle. Ordre de fusion 02 → 07 → 08 → 09, rebase sur
  `main` avant chaque merge.

## Décisions par chantier

### 03 — Un résolveur de cible de bibliothèque validé (*fusionné, #279*, [ADR 0009](./adr/0009-download-library-target-resolver.md))

Module privé `apps/api/src/downloads/library_target.rs` propriétaire de `resolve`,
`resolve_physical_root`, `find_existing_series_dir`, `validated_series_dir_name`. `target.rs` et
`import_pipeline.rs` l'appellent ; `target::resolve_target_dir` supprimé. La validation anti-traversée
ne s'applique qu'au fallback `<root>/<series_name>` (le nom de série vient d'une requête ou d'un
titre de release).

### 01 — Le cycle de vie des torrents dans un ledger gardé (*fusionné, #280*, [ADR 0010](./adr/0010-torrent-downloads-ledger.md))

`apps/api/src/downloads/torrent_ledger.rs` possède le vocabulaire des statuts (`TorrentStatus`),
toutes les écritures de `torrent_downloads`, les transitions gardées (`mark_completed`,
`begin_import`, `finish_import`, `fail`, `restart`) et `import_outcome` (qui décide
`imported` / `partial` / `no_files_imported`). `torrent_import.rs` et `alldebrid.rs` ne font plus
que lire, décider et déclencher les effets de bord (job de scan, événements, notifications).
Erreur typée `TorrentTransitionError` (`NotFound`, `NotAllowed`, `UnknownStatus`, `Database`)
convertie en `ApiError` dans les handlers, journalisée et ignorée dans les pollers.

### 02 — Un module de progression de transfert unique (*fusionné, #285*, [ADR 0011](./adr/0011-transfer-progress-module.md))

Quatre flux réimplémentaient progression, débit et ETA, avec trois formules d'ETA et deux
politiques `file_size` incompatibles (`rate.rs`, `alldebrid.rs`, `ddl_provider/service.rs`,
`telegram_monitor/transfer.rs`, `torrent_import.rs`).

`apps/api/src/downloads/progress.rs` possède désormais le throttle (`PROGRESS_UPDATE_BYTES`), un
`TransferProgress` unique (`from_bytes` / `from_reported` : une seule formule d'ETA), `TransferRate`
(ex-`rate.rs`), le trait `ProgressSink` et un `Reporter` par flux. Les sinks `TorrentSink`,
`DdlSink` et `TelegramSink` sont les seuls écrivains des colonnes de progression ;
`rate.rs` et `target.rs::DownloadProgress` sont supprimés.

Politique `file_size` retenue (elle inverse le contrat du chantier 01) : **la taille n'est jamais
effacée**. `None` et `Some(0)` (taille inconnue) conservent la valeur stockée ; seul un
`Some(n > 0)` la remplace.

### 07 — Un seam d'écriture unique pour les téléchargements (*fusionné, #286*, [ADR 0012](./adr/0012-download-write-seam.md))

`DownloadsPage.tsx` (quatre surfaces) réimplémentait le même protocole d'écriture : flags busy,
confirmation, routage du retry, refresh, `catch` local.

`apps/backoffice/lib/downloadActions.ts` (pur, sans React) porte le vocabulaire (`torrent`,
`alldebrid`, `ddl`, `telegram`, plus `release` et `entries` pour la section « disponibilités »), le
routage `actionFor(target, verb)` vers `browserApi`, la politique de confirmation comme donnée, un
état par ligne (`idle | pending(verb)`) qui coalesce les clics d'une même ligne, et une seule
politique d'échec : ligne remise à `idle`, erreur toastée, **pas** de refresh ; seul un succès
rafraîchit. `app/(app)/downloads/useDownloadActions.tsx` est l'adaptateur React
(`DownloadActionsProvider` / `useDownloadActions`) et possède l'unique dialogue de confirmation. Les
lignes (`DownloadRow`, `TelegramDownloadRow`, `DdlProviderDownloadRow`, `AvailableDownloadsSection`)
sont des adaptateurs de présentation. `DownloadClientButton` garde son flux d'ajout manuel.

### 08 — Un loader de détail de série, un view model (*fusionné, #283*, [ADR 0013](./adr/0013-series-detail-loader.md))

`app/(app)/series/[seriesId]/page.tsx` faisait dix-huit fetchs avec quatre politiques de repli
inline, et ce fan-out devenait l'interface à 29 props de `SeriesActionsToolbar`.

`apps/backoffice/lib/series/detail.ts` (serveur, sans React) expose `loadSeriesDetail(...)` et un
`SeriesDetailModel`, avec `{ status: "ok", model } | { status: "not-found" }`. Une seule politique
de dégradation : **fatal** (⇒ `not-found`) = la série et sa bibliothèque ; **non fatal** = tout le
reste (page de livres comprise, qui garde la grille vide d'avant) avec le nom de la section dans
`model.degraded`. La page ne fait plus que de la composition ; `SeriesActionsToolbar` reçoit un
unique prop `model`.

### 09 — Un descripteur de type de job et un seam SSE unique (*fusionné, #284*, [ADR 0014](./adr/0014-job-descriptors-and-sse-seam.md))

La connaissance des types de job était éclatée entre le catalogue serveur, une map locale
`JOB_TYPE_INFO`, neuf prédicats booléens et des fetchs de rapport par type ; trois composants
ouvraient leur propre `EventSource`.

`apps/backoffice/lib/jobs/descriptors.ts` est le registre unique (libellé, description, catégorie,
rejouabilité, rapport, plan de cartes), fusionné avec le catalogue serveur (autoritaire pour
`category` et `replayable`) ; les fetchers de rapports vivent dans `lib/jobs/serverReports.ts`,
injectés via `loaders`. `lib/useJobStream.ts`, construit sur `lib/useEventSource.ts`, est la seule
porte des flux SSE jobs (`useJobStream` pour le détail, `useJobsStream` pour la liste) ; plus aucun
composant n'instancie `EventSource`.

## Suivi

| Date | Événement |
|------|-----------|
| 2026-11-12 | Audit d'architecture, 10 chantiers identifiés |
| 2026-11-12 | #279 (chantier 03) fusionné |
| 2026-11-13 | #280 (chantier 01) fusionné |
| 2026-11-13 | #285 (chantier 02) fusionné |
| 2026-11-13 | #286 (chantier 07), #283 (chantier 08) et #284 (chantier 09) fusionnés |
| 2026-11-13 | ADR 0010–0014 ajoutées (chantiers 01, 02, 07, 08, 09) |
| 2026-11-13 | Second audit : 5 bugs (B1–B5), chantiers 11–25 ; vague 1 lancée |
| 2026-11-13 | Vague 1 ouverte : #289–#297 (ADR 0015–0021, sauf 0019 → #296) |
