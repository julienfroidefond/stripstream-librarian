## ADDED Requirements

### Requirement: Lier plusieurs providers à une série
Le système SHALL permettre d'approuver plusieurs liens de métadonnées pour une même série, un par provider (contrainte `UNIQUE (series_id, provider)` déjà en place). Approuver un lien NE DOIT PAS rejeter ni supprimer les autres liens approuvés de la série.

#### Scenario: Approuver un second provider conserve le premier
- **WHEN** une série possède un lien approuvé et que l'utilisateur approuve un lien d'un autre provider
- **THEN** les deux liens ont le statut `approved`
- **THEN** les `external_book_metadata` des deux liens sont conservés

#### Scenario: Premier lien approuvé devient principal
- **WHEN** l'utilisateur approuve un lien sur une série qui n'a aucun lien principal approuvé
- **THEN** ce lien est marqué `is_primary = true`

#### Scenario: Approuver un secondaire ne change pas le principal
- **WHEN** une série a déjà un lien principal approuvé et que l'utilisateur approuve un lien secondaire
- **THEN** le lien existant reste `is_primary = true` et le nouveau lien a `is_primary = false`

### Requirement: Un seul provider principal par série
Le système SHALL garantir au plus un lien principal (`is_primary = true`) parmi les liens approuvés d'une série, au niveau base de données.

#### Scenario: Contrainte d'unicité du principal
- **WHEN** une transaction tente de marquer un second lien approuvé comme principal sans démettre l'ancien
- **THEN** la base de données rejette l'opération (violation de l'index unique partiel)

### Requirement: Changer le provider principal
Le système SHALL exposer `PATCH /metadata/links/:id` acceptant `{ "is_primary": true }` pour promouvoir un lien approuvé en principal. La promotion SHALL démettre l'ancien principal dans la même transaction. L'opération SHALL relancer la synchronisation fusionnée de la série et retourner le lien mis à jour ainsi que le rapport de synchronisation.

#### Scenario: Promotion d'un secondaire en principal
- **WHEN** l'utilisateur envoie `PATCH /metadata/links/:id` avec `{ "is_primary": true }` sur un lien approuvé secondaire
- **THEN** ce lien passe `is_primary = true`
- **THEN** l'ancien principal passe `is_primary = false`
- **THEN** la série est re-synchronisée avec le nouveau principal prioritaire
- **THEN** la réponse HTTP 200 contient le lien et le rapport de synchronisation

#### Scenario: Lien non approuvé ne peut pas devenir principal
- **WHEN** l'utilisateur envoie `PATCH /metadata/links/:id` avec `{ "is_primary": true }` sur un lien `pending` ou `rejected`
- **THEN** le système retourne HTTP 422

#### Scenario: Lien inexistant
- **WHEN** l'utilisateur envoie `PATCH /metadata/links/:id` avec un UUID inexistant
- **THEN** le système retourne HTTP 404

### Requirement: Promotion automatique en cas de disparition du principal
Lorsqu'un lien principal est rejeté ou supprimé, le système SHALL promouvoir automatiquement le plus ancien lien approuvé restant de la série (par `approved_at ASC, id ASC`), s'il existe.

#### Scenario: Rejet du principal
- **WHEN** l'utilisateur rejette le lien principal d'une série qui possède au moins un autre lien approuvé
- **THEN** le plus ancien lien approuvé restant devient `is_primary = true`

#### Scenario: Suppression du principal
- **WHEN** l'utilisateur supprime le lien principal d'une série qui possède au moins un autre lien approuvé
- **THEN** le plus ancien lien approuvé restant devient `is_primary = true`

#### Scenario: Rejet du dernier lien approuvé
- **WHEN** l'utilisateur rejette le seul lien approuvé d'une série
- **THEN** la série n'a plus aucun lien principal (`is_primary = false` pour tous)

### Requirement: Fusion des métadonnées de série avec repli sur les secondaires
La synchronisation SHALL fusionner les métadonnées de série champ par champ : pour chaque champ, la valeur du provider principal est utilisée en priorité ; si elle est absente ou vide, la première valeur non vide parmi les secondaires (ordonnés par `approved_at ASC, id ASC`) est utilisée. Les champs `locked_fields` de la série NE DOIVENT PAS être écrasés. Les genres sont hors du périmètre de fusion (voir exigence dédiée).

#### Scenario: Champ présent chez le principal
- **WHEN** le principal fournit une `description` et qu'un secondaire en fournit une autre
- **THEN** la `description` du principal est conservée

#### Scenario: Champ absent chez le principal, présent chez un secondaire
- **WHEN** le principal ne fournit pas de `start_year` et qu'un secondaire en fournit un
- **THEN** le `start_year` du secondaire est appliqué

#### Scenario: Champ absent chez tous les providers
- **WHEN** aucun provider (principal ou secondaire) ne fournit `total_volumes`
- **THEN** la valeur existante de la série est conservée

#### Scenario: Champ verrouillé
- **WHEN** un champ est marqué verrouillé dans `series.locked_fields` et que la fusion produit une nouvelle valeur
- **THEN** la valeur existante de la série est conservée et le champ apparaît dans `fields_skipped` du rapport

#### Scenario: Genres hors fusion
- **WHEN** le principal ne fournit aucun genre et qu'un secondaire en fournit, sur une série ayant déjà des genres
- **THEN** les genres existants de la série sont conservés

### Requirement: Fusion des métadonnées de tomes avec repli sur les secondaires
La synchronisation des tomes SHALL fusionner, pour chaque tome local, les données du provider principal avec celles des secondaires : le tome du principal est prioritaire, et chaque champ manquant (`summary`, `isbn`, `publish_date`, `language`, `authors`) est complété par un secondaire. Les `locked_fields` du livre NE DOIVENT PAS être écrasés.

#### Scenario: Résumé du tome présent chez le principal
- **WHEN** le principal fournit le résumé du tome 3 et qu'un secondaire en fournit un autre
- **THEN** le résumé du principal est appliqué au livre

#### Scenario: Résumé absent chez le principal
- **WHEN** le principal ne fournit pas le résumé du tome 3 et qu'un secondaire en fournit un
- **THEN** le résumé du secondaire est appliqué au livre

#### Scenario: ISBN complété par un secondaire
- **WHEN** le principal ne fournit pas d'ISBN pour un tome et qu'un secondaire en fournit un
- **THEN** l'ISBN du secondaire est appliqué

### Requirement: Provider affiché dans la liste des séries
La liste des séries SHALL afficher le provider du lien principal approuvé lorsque plusieurs liens sont approuvés.

#### Scenario: Série à plusieurs providers
- **WHEN** une série a un lien principal BdTheque et un lien secondaire BDPhile
- **THEN** la liste des séries affiche `bdtheque` comme provider de métadonnées

### Requirement: Synchronisation globale respectant la priorité
Le job de rafraîchissement des métadonnées SHALL traiter tous les liens approuvés d'une série et appliquer la fusion avec le principal prioritaire, quel que soit l'ordre d'itération.

#### Scenario: Refresh avec plusieurs liens
- **WHEN** un job de refresh parcourt une série à plusieurs liens approuvés
- **THEN** la série et ses tomes sont synchronisés selon la fusion principal-prioritaire
- **THEN** le résultat ne dépend pas de l'ordre de traitement des liens

### Requirement: Exposer `is_primary` dans l'API des liens
Le DTO des liens de métadonnées (`ExternalMetadataLinkDto`) SHALL exposer le champ `is_primary`.

#### Scenario: Lecture des liens d'une série
- **WHEN** le client appelle `GET /metadata/links?series_id=…`
- **THEN** chaque lien retourné contient `is_primary` (booléen)
