## Context

`external_metadata_links` porte déjà la contrainte `UNIQUE (series_id, provider)` (migration `0071`) : le schéma autorise donc **plusieurs providers par série**. Le blocage est applicatif : `approve_metadata` (`apps/api/src/metadata/handlers.rs`) rejette tous les autres liens approuvés de la série.

La synchro est déjà « nouvelle valeur non vide gagne, sinon on garde l'existant », via `upsert_series_metadata` (`shared_sync.rs`) :

- `description`, `cover_url` → `COALESCE(NULLIF(EXCLUDED.x,''), series.x)`
- `start_year`, `total_volumes`, `status` → `COALESCE(EXCLUDED.x, series.x)`
- `authors`, `publishers` → remplacés si non vides
- `genres` → remplis **uniquement si la série n'en a aucun** (seed)
- `locked_fields` (JSON) → champs verrouillés non écrasés

`push_book_metadata` suit la même logique `COALESCE(new, existing)` pour `summary`, `isbn`, `publish_date`, `language`, `authors`.

Les notes communautaires (`series/list.rs`, `series/ratings.rs`) agrègent **déjà** sur tous les liens `status='approved'` : activer plusieurs liens donnera donc gratuitement une `community_score` multi-providers.

## Goals / Non-Goals

**Goals:**
- Autoriser plusieurs liens approuvés par série.
- Désigner un provider principal par série (au niveau du lien).
- Fusionner les métadonnées (série + tomes) : principal prioritaire, secondaires en repli champ par champ.
- Rendre le changement de principal immédiatement effectif.
- Rester rétro-compatible (une série avec un seul lien approuvé = ce lien principal).

**Non-Goals:**
- Fusion des **genres** par priorité : les genres restent pilotés par l'app (AI tagging / manuel). Comportement actuel conservé (seed uniquement si la série n'a aucun genre).
- Chaîne de priorité ordonnée à plus de deux niveaux (principal + N secondaires non ordonnés). Une colonne `priority SMALLINT` pourra être ajoutée plus tard si besoin.
- Multi-utilisateur / droits par provider.
- Fusion des images de couverture (le principal gagne s'il a une cover, sinon repli secondaire — via `COALESCE` existant).

## Decisions

### D1 — `is_primary BOOLEAN` sur le lien, pas de table dédiée

Ajout de `is_primary BOOLEAN NOT NULL DEFAULT false` sur `external_metadata_links`, plus un index unique partiel :

```sql
CREATE UNIQUE INDEX external_metadata_links_one_primary_per_series
    ON external_metadata_links (series_id)
    WHERE is_primary AND status = 'approved';
```

**Rationale** : le besoin exprimé est « un principal par série ». Un booléen est fidèle, simple, et l'index partiel garantit l'invariant au niveau DB (pas seulement applicatif). Les secondaires sont ordonnés de façon déterministe par `approved_at ASC, id ASC` (premier approuvé gagne en cas de conflit entre secondaires).

**Alternative rejetée** : `priority SMALLINT` (0 = principal). Plus flexible (chaîne ordonnée) mais complexité inutile tant que le besoin n'est pas exprimé. Migration triviale depuis `is_primary` le jour venu.

**Alternative rejetée** : `series.primary_provider TEXT`. Moins précis (le provider n'identifie pas le lien), et casse si le lien est remplacé.

### D2 — Fusion explicite champ par champ (série et tomes)

Plutôt que de synchroniser chaque lien séquentiellement (où le dernier écrase), on fusionne explicitement :

- `merge_series_fields(links: &[LinkMetadata]) -> SeriesFields` : les liens sont triés **principal d'abord**, puis secondaires ; pour chaque champ on prend la **première valeur non vide**.
- Fusion des tomes : pour chaque tome local, on part du `BookCandidate` du lien principal et on complète les champs manquants (`summary`, `isbn`, `publish_date`, `language`, `authors`) avec ceux des secondaires.
- Un **seul** `upsert_series_metadata` / `push_book_metadata` par série/tome, avec la valeur fusionnée.

**Rationale** : sémantique claire et testable (« le principal gagne, le secondaire comble »), indépendante de l'ordre de traitement des liens et du hasard des `COALESCE`. Évite les réécritures multiples en DB et l'accumulation d'états intermédiaires.

**Alternative rejetée** : traiter les liens dans l'ordre secondaires → principal en s'appuyant sur la sémantique `COALESCE` existante. Moins de code, mais fragile (dépend de l'ordre d'exécution, cas particulier des genres, difficile à tester) et ne permet pas de construire un rapport de diff propre.

### D3 — `is_primary` attribué automatiquement, changeable via `PATCH`

- À l'approbation : si la série n'a aucun principal approuvé, le lien approuvé devient principal automatiquement. Le body `ApproveRequest` accepte un `is_primary: bool` optionnel pour forcer.
- Au rejet / à la suppression du principal : promotion automatique du plus ancien lien approuvé restant (s'il existe), pour préserver l'invariant « un principal dès qu'un lien est approuvé ».
- `PATCH /metadata/links/:id` avec `{ "is_primary": true }` : démote l'ancien principal et promeut ce lien dans une transaction, puis relance la synchro fusionnée de la série (`sync_series` + `sync_books`, activables via le body, `true` par défaut). Retourne le lien mis à jour + le `SyncReport`.

**Rationale** : le premier provider lié est presque toujours le bon principal (pas de friction). Le `PATCH` permet de corriger sans re-approuver, et déclenche la synchro pour que le changement soit visible immédiatement.

### D4 — `approve` synchronise toute la série, plus seulement le lien

`approve_metadata` (et le `PATCH`) appellent `sync_series_from_links(state, series_id, …)` : récupère tous les liens approuvés, les trie, fusionne, synchronise. Le rapport renvoyé reste un `SyncReport` unique.

**Rationale** : l'ordre relatif des liens compte ; synchroniser un seul lien laisserait la série dans un état incohérent (par ex. approuver un secondaire après coup écraserait le principal). La synchro série complète garantit l'idempotence.

### D5 — Genres hors du périmètre de fusion

Les genres ne participent pas à la priorité. On conserve le comportement existant : un provider ne renseigne les genres que si la série n'en a aucun (valeur issue du principal dans la fusion), l'app restant la source de vérité.

**Rationale** : décision produit — « la synchro ne gère pas les genres, c'est le site qui le fait ».

### D6 — Provider affiché = le principal

Les requêtes de `series/list.rs` qui sélectionnent `eml.provider` (LATERAL `ORDER BY eml.created_at DESC`) passent à `ORDER BY eml.is_primary DESC, eml.created_at DESC LIMIT 1`.

**Rationale** : avec plusieurs liens, le provider affiché doit être le principal, pas le plus récent.

## Risks / Trade-offs

- **Plus d'appels providers par série** : la synchro/refresh appelle `get_series_books` pour chaque lien approuvé (comportement déjà vrai au refresh aujourd'hui pour N liens). Acceptable ; le refresh traite déjà lien par lien.
- **Volume de données `external_book_metadata`** : une ligne par tome et par lien. Déjà le cas par construction (`link_id`), pas de changement structurel.
- **Contrainte d'unicité du principal** : promotion/démotion doivent être transactionnelles, sinon violation d'index. À couvrir par un test DB.
- **Changement de comportement d'approve** : le rejet automatique des autres liens disparaît. Les séries existantes (un seul lien approuvé) deviennent `is_primary = true` via la migration (backfill), donc aucun changement visible.
- **UI** : la modale de matching et la toolbar supposent un lien unique (`existingLink`). À adapter pour afficher/choisir parmi plusieurs.

## Migration Plan

1. Migration `0117_add_primary_metadata_link.sql` :
   - `ALTER TABLE external_metadata_links ADD COLUMN is_primary BOOLEAN NOT NULL DEFAULT false;`
   - Backfill : marquer `is_primary = true` sur le lien approuvé de chaque série qui en a un (au plus un aujourd'hui) :
     ```sql
     UPDATE external_metadata_links eml SET is_primary = true
     WHERE eml.status = 'approved'
       AND eml.id = (
         SELECT e2.id FROM external_metadata_links e2
         WHERE e2.series_id = eml.series_id AND e2.status = 'approved'
         ORDER BY e2.approved_at ASC NULLS LAST, e2.id ASC
         LIMIT 1
       );
     ```
   - Créer l'index unique partiel.
2. Déployer l'API (approve multi-liens, PATCH, fusion).
3. Déployer le backoffice (badge principal, action PATCH).
4. Rollback : `DROP INDEX`, `DROP COLUMN` (le down file) — les séries retombent sur le comportement mono-lien ; aucune donnée de contenu perdue (les liens approuvés multiples resteraient, mais l'ancien binaire n'en exploiterait qu'un).

## Open Questions

- Le filtre `metadata_provider_cond` de la liste des séries doit-il matcher n'importe quel lien approuvé ou seulement le principal ? (À trancher à l'implémentation ; proposition : n'importe quel lien approuvé, pour ne pas casser les filtres existants.)
