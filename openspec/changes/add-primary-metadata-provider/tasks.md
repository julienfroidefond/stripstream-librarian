## 1. Migration DB

- [x] 1.1 Créer `infra/migrations/0117_add_primary_metadata_link.sql` : `ALTER TABLE external_metadata_links ADD COLUMN is_primary BOOLEAN NOT NULL DEFAULT false`
- [x] 1.2 Backfill : marquer `is_primary = true` le lien approuvé le plus ancien de chaque série (celui qui a un lien approuvé)
- [x] 1.3 Créer l'index unique partiel `external_metadata_links_one_primary_per_series ON external_metadata_links(series_id) WHERE is_primary AND status = 'approved'`
- [x] 1.4 Créer `infra/migrations/0117_add_primary_metadata_link.down.sql` (drop index + colonne)
- [x] 1.5 Vérifier `sqlx migrate run` puis `sqlx migrate revert` en local (migration appliquée par `#[sqlx::test]`)

## 2. Fusion dans shared_sync.rs

- [x] 2.1 Ajouter la struct `LinkMetadata { is_primary, provider, metadata_json, total_volumes_external }` et un helper d'ordonnancement (principal d'abord, puis `approved_at ASC, id ASC`)
- [x] 2.2 Implémenter `merge_series_fields(links: &[LinkMetadata]) -> SeriesFields` : première valeur non vide par champ (`description`, `authors`, `publishers`, `start_year`, `total_volumes`, `status`, `cover_url`) ; genres = valeur du principal (hors fusion)
- [x] 2.3 Implémenter la fusion des tomes : `merge_book_rows` (principal prioritaire, complétion des champs manquants par les secondaires)
- [x] 2.4 Implémenter `sync_series_from_links(pool, series_id, sync_series, sync_books)` : charger les liens approuvés, trier, fusionner, un seul upsert série, insérer `external_book_metadata` par lien, pousser les tomes fusionnés une fois
- [x] 2.5 Ajouter les tests unitaires de `merge_series_fields` (principal gagne, fallback, tous vides, genres ignorés) et de `merge_book_rows` (fallback par champ)

## 3. Approve multi-liens + PATCH principal (handlers.rs)

- [x] 3.1 Dans `approve_metadata`, supprimer le rejet des autres liens approuvés (`UPDATE ... status='rejected'` + `DELETE FROM external_book_metadata`)
- [x] 3.2 À l'approbation, si la série n'a aucun principal approuvé, marquer le lien `is_primary = true` (ou honorer `body.is_primary`)
- [x] 3.3 Remplacer la synchro mono-lien par `sync_series_from_links(...)` (série + tomes fusionnés)
- [x] 3.4 Au rejet (`reject_metadata`) et à la suppression (`delete_metadata_link`), promouvoir automatiquement le plus ancien lien approuvé restant s'il existe (transaction)
- [x] 3.5 Ajouter le handler `patch_metadata_link` (`PATCH /metadata/links/:id`) : valider le lien approuvé (422 sinon), démote l'ancien principal + promeut le nouveau (transaction), relance `sync_series_from_links`, retourne lien + rapport
- [x] 3.6 Ajouter `is_primary` à `ExternalMetadataLinkDto` et à `row_to_link_dto`, ainsi qu'aux `SELECT` (`get_metadata_links`, re-fetch de `create_metadata_match`)
- [x] 3.7 Annoter `patch_metadata_link` avec `#[utoipa::path(...)]` (tag "metadata", body, réponses 200/404/422, security Bearer)

## 4. Routes et OpenAPI

- [x] 4.1 Déclarer la route `PATCH /metadata/links/:id` dans `apps/api/src/main.rs` (section admin, cohérente avec les autres mutations metadata)
- [x] 4.2 Enregistrer `patch_metadata_link` dans `AdminApiDoc` (`openapi.rs`) et le schema `PatchLinkRequest`/`PatchLinkResponse`
- [x] 4.3 Mettre à jour le `ExternalMetadataLinkDto` exposé dans le spec (champ `is_primary`)

## 5. Refresh et liste des séries

- [x] 5.1 Dans `refresh.rs`/`refresh_sync.rs`, traiter tous les liens approuvés d'une série et fusionner (série + tomes) plutôt que `refresh_link` mono-lien
- [x] 5.2 Dans `series/list.rs`, sélectionner le provider principal : `ORDER BY eml.is_primary DESC, eml.created_at DESC LIMIT 1` (les deux requêtes)
- [x] 5.3 Vérifier le comportement du filtre `metadata_provider_cond` avec plusieurs liens approuvés (matcher n'importe quel lien approuvé) et le corriger si besoin

## 6. Tests API

- [x] 6.1 Test DB : approuver deux providers conserve les deux liens approuvés (plus de rejet automatique)
- [x] 6.2 Test DB : le premier lien approuvé devient principal ; approuver un secondaire ne change pas le principal
- [x] 6.3 Test DB : contrainte d'unicité — impossible d'avoir deux principaux approuvés
- [x] 6.4 Test DB : `PATCH` promeut un secondaire, démote l'ancien, et re-synchronise la série
- [x] 6.5 Test DB : rejet/suppression du principal promeut le plus ancien approuvé restant
- [x] 6.6 Test DB : fusion série — le principal gagne, un champ manquant est comblé par un secondaire, les `locked_fields` sont respectés
- [x] 6.7 Test DB : fusion tomes — résumé/ISBN du principal prioritaires, complétés par un secondaire
- [x] 6.8 Test DB : les genres existants ne sont pas écrasés par la fusion
- [x] 6.9 Test unitaire : `is_primary` présent dans `GET /metadata/links`

## 7. Backoffice

- [x] 7.1 `lib/api.ts` : ajouter `is_primary` à `ExternalMetadataLinkDto` et la fonction `setMetadataLinkPrimary(id, isPrimary, opts)`
- [x] 7.2 `app/api/metadata/links/route.ts` : ajouter le handler `PATCH` (proxy vers l'API)
- [x] 7.3 Fiche série `app/(app)/series/[seriesId]/page.tsx` : afficher tous les liens approuvés avec un badge « Principal » et une action pour définir le principal
- [x] 7.4 `MetadataSearchModal.tsx` / `SeriesActionsToolbar.tsx` : adapter la logique qui suppose un seul `existingLink` (afficher/choisir parmi plusieurs)
- [x] 7.5 Tests : route handler PATCH + composant badge principal / sélection du principal

## 8. Documentation

- [x] 8.1 Mettre à jour `apps/docs/src/content/docs/metadata/providers.md` : section « Provider principal et repli » (plusieurs liens par série, priorité, fusion, genres pilotés par l'app)
- [x] 8.2 Mentionner que la note communautaire agrège désormais naturellement plusieurs providers approuvés

## 9. Vérifications finales

- [x] 9.1 `cargo fmt -- --check` et `cargo clippy --workspace`
- [x] 9.2 `cargo test --workspace` (unitaires)
- [x] 9.3 `DATABASE_URL=... cargo test --workspace` (tests DB)
- [x] 9.4 `cd apps/backoffice && npx tsc --noEmit && npm run lint && npm run test:unit`
- [ ] 9.5 Test manuel : lier deux providers à une série, définir le principal, vérifier la fusion (série + tomes)
