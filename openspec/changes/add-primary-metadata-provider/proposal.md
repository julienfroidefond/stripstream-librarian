## Why

Aujourd'hui, `approve_metadata` (`apps/api/src/metadata/handlers.rs`) rejette automatiquement tous les autres liens approuvés d'une série : une série ne peut avoir qu'un seul provider de métadonnées actif. Or les providers sont complémentaires (BDTheque/BDPhile/Bdgest pour la BD, AniList pour le manga, Google Books/OpenLibrary pour les ISBN…). Quand le provider retenu n'a pas une information (description, année de début, nombre de tomes, résumé d'un tome…), cette information reste absente alors qu'un autre provider la possède.

L'objectif est de pouvoir lier une série à **plusieurs providers** et de désigner un **provider principal** : la synchro prend toujours l'information du principal, et si elle manque, va la chercher chez les secondaires.

## What Changes

- Nouvelle colonne `is_primary` sur `external_metadata_links`, avec index unique partiel garantissant **au plus un lien principal approuvé par série**.
- `POST /metadata/approve/:id` n'invalide plus les autres liens approuvés : plusieurs providers peuvent coexister sur une même série. Le premier lien approuvé devient principal par défaut.
- Nouvel endpoint `PATCH /metadata/links/:id` pour changer le provider principal (promotion d'un lien, l'ancien principal devenant secondaire) et relancer la synchro fusionnée.
- Synchro **fusionnée** : les métadonnées de série sont fusionnées champ par champ, principal prioritaire, secondaires comblant les trous. Les métadonnées de tomes suivent la même règle (le tome du principal gagne, fallback secondaire par champ).
- La liste des séries affiche le provider **principal** (au lieu du plus récemment créé).
- Backoffice : badge « Principal », action pour définir/retirer le principal, plusieurs liens approuvés visibles sur la fiche série.
- Documentation mise à jour (provider principal + fallback).

## Capabilities

### New Capabilities

- `metadata-provider-priority` : lier une série à plusieurs providers de métadonnées et désigner un principal ; fusionner les métadonnées avec repli sur les secondaires.

### Modified Capabilities

- (aucune capacité existante impactée ; `cbr-conversion` n'est pas concernée)

## Impact

- **DB** : nouvelle migration `0117_add_primary_metadata_link.sql` (+ `.down.sql`) — colonne `is_primary`, index unique partiel.
- **API** :
  - `apps/api/src/metadata/handlers.rs` — `approve_metadata` (plus de rejet des autres liens, attribution auto du principal), nouvel handler `patch_metadata_link`, `ExternalMetadataLinkDto` + `is_primary`, `row_to_link_dto`.
  - `apps/api/src/metadata/shared_sync.rs` — `merge_series_fields` + fusion des tomes (`merge_book_candidates`), `sync_series_from_links`.
  - `apps/api/src/metadata/refresh.rs` / `refresh_sync.rs` — traiter tous les liens approuvés d'une série selon la priorité.
  - `apps/api/src/metadata/sync.rs` — synchro série + tomes fusionnée.
  - `apps/api/src/series/list.rs` — sélection du provider principal (`is_primary DESC`).
  - `apps/api/src/main.rs` + `openapi.rs` — route et schéma du `PATCH`.
- **Backoffice** : `lib/api.ts`, `app/api/metadata/links/route.ts` (PATCH), fiche série `app/(app)/series/[seriesId]/page.tsx`, `MetadataSearchModal.tsx`, `SeriesActionsToolbar.tsx`.
- **Docs** : `apps/docs/src/content/docs/metadata/providers.md`.
