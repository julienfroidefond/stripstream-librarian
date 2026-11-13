# Un module propriétaire de `external_metadata_links`

`external_metadata_links` était écrit depuis le match manuel, l'auto-match batch, la création de
série, le refresh, la fusion de séries et trois chemins de retrait. Les trois upserts avaient des
sémantiques implicites et divergentes (la création de série gardait `external_id` et
`external_url` mais écrasait données et notes, sans mettre à jour les notes de notation), chaque
retrait appliquait sa propre politique, et la fusion déplaçait les liens
sans tenir compte de `is_primary` : deux séries ayant chacune un primaire violaient l'index
`external_metadata_links_one_primary_per_series` et la fusion échouait.

`metadata::links` possède désormais toutes les écritures de la table : `propose` (→ `pending`),
`approve` (upsert → `approved`), `approve_existing`, `set_primary`, `remove(Removal::{Delete,
Reject, ProvidersNotIn})` → `AfterRemoval`, `reassign(tx, from, to)`, `record_provider_snapshot` et
`mark_synced`. Il porte l'invariant « un seul primaire approuvé par série », l'extraction des notes
(`ProviderNotes`) et une erreur typée `LinkError` (`NotFound`, `NotApproved`, `Database`, `Sync`)
convertie en `ApiError`.

**Politique après retrait (unique).** Quand un retrait sort au moins un lien *approuvé* de
l'ensemble approuvé (suppression, rejet, nettoyage rematch, ou re-proposition d'un lien approuvé),
le plus ancien lien approuvé restant est promu si la série a perdu son primaire, puis les champs de
la série sont refusionnés depuis les liens stockés restants (sans appel provider), ou effacés via
`clear_series_metadata` s'il n'en reste aucun. Retirer un lien `pending` ou `rejected` ne touche à
rien d'autre. C'est la politique de la suppression, généralisée : le rejet et le rematch laissaient
sur la série des champs venant d'un lien qui ne compte plus.

## Conséquences

- Aucun `INSERT`, `UPDATE` ou `DELETE` sur `external_metadata_links` hors de `metadata/links.rs`
  dans le code de production de l'API (les tests gardent leurs fixtures SQL).
- Deux sémantiques d'upsert approuvé subsistent, mais explicites (`OnConflict`) : `Replace`
  (auto-match : la nouvelle entrée provider remplace identité, données et notes) et `KeepExisting`
  (création de série / Discovery : deux entités de même titre tombent sur une série, l'identité
  stockée est conservée, cf. #25). `KeepExisting` garde aussi les données et notes de cette
  identité, alors qu'avant la ligne mélangeait l'`external_id` d'origine et les données de la
  nouvelle entité. Dans les deux cas un primaire approuvé le reste et un drapeau primaire résiduel
  sur une ligne non approuvée est effacé.
- Supprimer un lien `pending` n'efface plus les champs de la série (avant : effacement si aucun
  lien approuvé ne restait).
- La fusion conserve le primaire de la cible ; un primaire déplacé devient secondaire, et la cible
  sans primaire en reçoit un.
- L'auto-match et le nettoyage rematch résolvent la série via `series_identity` (ADR 0007).
