# Tests E2E — Backoffice

Suite Playwright organisée par **domaines fonctionnels**. Chaque dossier correspond à un grand
domaine du backoffice ; les specs couvrent le comportement observable (navigation, formulaires,
modales, états vides) sans dépendre d'un ordre d'exécution.

## Prérequis

- API (`:7080`), indexer et PostgreSQL démarrés (stack Docker) ;
- backoffice sur `:7082` (`npm run dev`, lancé automatiquement par Playwright si absent) ;
- `ADMIN_PASSWORD` (et éventuellement `ADMIN_USERNAME`) dans `apps/backoffice/.env.local` ;
- `API_BOOTSTRAP_TOKEN` dans le même fichier pour le nettoyage via l'API Rust.

```bash
npm run test:e2e         # headless
npm run test:e2e:headed  # navigateur visible
npm run test:e2e:ui      # mode UI
npx playwright test catalog/series.spec.ts -g "recherche"
```

## Architecture

| Élément | Rôle |
|---------|------|
| `auth.setup.ts` | Projet `setup` : connexion admin, création/réutilisation du lecteur `e2e-reader`, nettoyage des ressources `e2e-*` orphelines, préparation des bibliothèques temporaires (`e2e-metadata`, `e2e-books`, `e2e-jobs`), sauvegarde de `storageState` + cookie `as_user_id`. |
| `env.ts` | Lecture de `.env.local` (identifiants admin). |
| `helpers.ts` | `openFirstSeries`, `openSeriesDetail`, `uniqueName`, `escapeRegExp`. |
| `support/api.ts` | Appels directs à l'API Rust avec le token bootstrap : feature flags, nettoyage listes/tokens/users, CRUD bibliothèques/séries, liens et métadonnées de série, notes, jobs, archives et progression de lecture (`X-As-User`). |
| `support/library.ts` | Seed local optionnel (`E2E_SEED_LIBRARY=1`) : copie les CBZ de `fixtures/library/`, crée la bibliothèque via l'API, déclenche un scan et attend l'indexation. |
| `support/metadata.ts` | Bibliothèques temporaires `e2e-metadata`, `e2e-books` et `e2e-jobs` : séries de test avec auteurs/genres uniques, nettoyage, purge des archives et helper `openMetadataSeries` (liste → fiche, avec réchauffement du cache des bibliothèques). |
| `support/books.ts` | Livre ou série jetable multi-tomes : copie un ou plusieurs CBZ dans `e2e-books`, déclenche un scan et attend que l'indexer expose les lignes via `GET /books`. |
| `global-teardown.ts` | Suppression des bibliothèques temporaires et de leurs archives en fin de run. |
| `fixtures/library/` | Mini-bibliothèque déterministe (série `E2E Series`, tomes 1 et 2) utilisée par le seed local et par les livres jetables. |
| `tests/e2e/.auth/state.json` | Session admin générée par le setup (ignoré par git). |

L'interface est en **anglais** par défaut ; seules les libellés de la page de connexion sont codés
en français. Le cookie `as_user_id` active les UI « utilisateur actif » sans changer la session
admin.

## Matrice des domaines

| Domaine | Dossier | Couvert | Non couvert volontairement |
|---------|---------|---------|----------------------------|
| Shell / navigation | `shell/` | Rendu des 15 routes, menus desktop et tiroir mobile, liens directs, thème clair/sombre, pastille jobs, redirection auth, login invalide/valide, logout, modale générique (Escape / bouton). | Bascule de langue (réglage global persistant). |
| Dashboard | `dashboard/` | Cartes de statistiques, répartitions, sélecteurs de période des graphiques. | — |
| Catalogue | `catalog/` | Recherche séries (`?q=`), édition de tags (ajout/retrait sans sauvegarde), modale métadonnées, confirmation de suppression annulée, **renommage réel + suppression via Actions et vérification de l'archive avec auteurs/genres**, **édition de champs + verrouillage/déverrouillage**, **fusion de deux séries avec déplacement du lien de métadonnées**, **notation en demi-étoiles persistée puis retirée**, **actions livre (édition du titre, téléchargement, suppression annulée puis confirmée, verrouillage/déverrouillage d'un champ)**, lecture (bouton, volumes), liste/détail livre + infos techniques, recherche vide, auteurs (recherche + fiche via fixture), genres (stats/filtres, **affectation en masse depuis le filtre « sans genre », recherche insensible aux accents et suppression réelle via fixture**). | Renommage de fichiers par template, conversion CBR → CBZ, séries liées et synchronisation via providers externes. |
| Métadonnées | `metadata/` | Onglets Séries/Livres, filtre par gap, compteur de résultats, recherche sans résultat, **promotion d'un lien approuvé et synchronisation des auteurs/genres/description depuis `metadata_json`**, **recherche provider mockée : approbation, synchronisation « série seule » puis déliaison**. | Recherche et rafraîchissement via un provider externe réel ; volumes manquants (`external_book_metadata` non seedable par l'API publique). |
| Lecture | `reading/` | Progression de l'utilisateur actif (vide ou peuplée), **marquage lu/non lu avec persistance après rechargement et restauration de l'état d'origine via l'API**, **marquage de toute une série avec vérification du statut et du compteur par tome**, cycle complet liste de lecture : création, ajout/retrait de série, suppression (dialogue natif), **renommage inline, réordonnancement et groupement `?group_by=reading_list`**, nettoyage API. | Suggestions IA, provider de statut de lecture, vue admin transversale multi-utilisateurs. |
| Bibliothèques | `libraries/` | Cartes, formulaire d'ajout, navigation vers les séries et les livres d'une bibliothèque, **réglages persistés (scan, watcher, providers, tags) puis restaurés via l'API**. | Création/suppression réelle de bibliothèque (volume hôte). |
| Acquisition | `acquisition/` | Drapeau `downloads_enabled` (page ou redirection), découverte : onglets Trending/Wishlist, panneau des séries masquées, **série à zéro livre visible dans la wishlist et filtres livres de la fiche série**. | Prowlarr/Telegram/DDL et téléchargements réels (configuration externe). |
| Opérations | `operations/` | Jobs : filtres, détail, palette d'actions (recherche, Échap), **lancement d'un rebuild ciblé puis replay avec vérification des statuts terminaux**. Diagnostics : indicateurs, sections, rafraîchissement, **purge du cache avec vérification de la réponse et du message affiché**. | Annulation de job, régénération globale des miniatures, téléchargements réels. |
| Admin | `admin/` | Matrice des onglets Paramètres, présence des tests de connexion, **tests Prowlarr, qBittorrent, Telegram et DDL mockés (AllDebrid seulement si configuré)**, CRUD token (création → révocation → suppression), **réassignation de token et renommage inline d'un lecteur**, CRUD utilisateur lecteur, **bascule d'utilisateur actif + restrictions de genres (série masquée puis réaffichée pour Admin)**, **fixtures déterministes Orphan series → Archives, suppression unitaire annulée puis confirmée et annulation de la suppression en masse**. | Enregistrement de réglages globaux, confirmation de la suppression en masse (supprimerait les fixtures des autres workers), connexions externes réelles. |

## Backlog — parcours documentés non couverts

Recensement issu de `apps/docs/`. Le tableau distingue les parcours désormais couverts, ceux qui
restent à implémenter, les blocages techniques et les exclusions volontaires. Les providers
externes réels, les réglages globaux et les règles métier pures (matching ISBN, restauration
d'archives, internes de conversion) restent hors périmètre E2E.

| Priorité | Parcours | Doc | Spec | Statut |
|----------|----------|-----|------|--------|
| Haute | Actions livre : édition, téléchargement, suppression (dernier livre → série archivée) | `books/download`, `books/deletion`, `books/metadata` | `catalog/books-write.spec.ts` | **Couvert** |
| Haute | Verrouillage puis déverrouillage d'un champ d'un livre | `books/metadata`, `metadata/sync` | `catalog/books-write.spec.ts` | **Couvert** |
| Haute | Édition, verrouillage et déverrouillage des champs d'une série | `series/management`, `metadata/sync` | `catalog/series-write.spec.ts` | **Couvert** |
| Haute | Fusion de deux séries et déplacement du lien de métadonnées | `series/management` | `catalog/series-merge.spec.ts` | **Couvert** |
| Haute | Notation d'une série par le lecteur actif | `series/ratings` | `catalog/ratings.spec.ts` | **Couvert** |
| Haute | Réglages d'une bibliothèque (scan, watcher, providers, tags) | `libraries/management` | `libraries/settings-write.spec.ts` | **Couvert** |
| Haute | Recherche mockée, approbation, synchronisation « série seule » et déliaison | `metadata/sync`, `metadata/providers` | `metadata/search-approve.spec.ts` | **Couvert** (provider simulé) |
| Haute | Affectation en masse d'un genre, filtre « sans genre » et suppression du genre | `series/genres` | `catalog/authors-genres.spec.ts` | **Couvert** |
| Haute | Marquage de toute une série comme lue avec statut et compteur par tome | `series/reading-progress` | `reading/progress-write.spec.ts` | **Couvert** |
| Quick win | Réassignation d'un token et renommage inline d'un utilisateur | `reference/users-tokens` | `admin/tokens.spec.ts` | **Couvert** |
| Quick win | Suppression unitaire d'une série orpheline (annulation puis confirmation) | `series/orphans` | `admin/archives-orphans.spec.ts` | **Couvert** |
| Quick win | Affichage et dépliage des séries archivées | `series/archives` | `admin/archives-orphans.spec.ts` | **Couvert** |
| Quick win | Purge du cache depuis les diagnostics | `getting-started/diagnostics` | `operations/diagnostics.spec.ts` | **Couvert** |
| Quick win | Tests de connexion Prowlarr, qBittorrent, Telegram, DDL et AllDebrid mockés | `integrations/*`, `downloads/*` | `admin/connection-test.spec.ts` | **Couvert** (DDL/AllDebrid selon configuration) |
| Quick win | Renommage, réordonnancement et groupement des listes de lecture | `series/reading-lists` | `reading/lists.spec.ts` | **Couvert** |
| Quick win | Wishlist : série à zéro livre et filtres de la fiche série | `discovery/wishlist` | `acquisition/discovery.spec.ts` | **Couvert** |
| Haute | Volumes manquants et bascule d'affichage | `series/missing-volumes` | `metadata/missing-volumes.spec.ts` | **Bloqué** : aucune API publique ne seed `external_book_metadata` |
| Haute | Renommage des fichiers d'une série via templates (aperçu, type/tome) | `books/renaming` | `catalog/series-rename.spec.ts` | **À faire** |
| Moyenne | Génération des miniatures manquantes et régénération complète | `books/thumbnails`, `jobs/miniatures` | `operations/jobs-thumbnails.spec.ts` | **À faire** |
| Moyenne | Annulation d'un job en cours | `jobs/overview` | `operations/jobs-write.spec.ts` | **À faire** (job long contrôlable requis) |
| Moyenne | Séries liées et recommandations personnalisées | `series/related` | `catalog/series-related.spec.ts` | **À faire** |
| Moyenne | Vue admin de progression multi-utilisateurs (détail par série) | `series/reading-progress` | `reading/progress-admin.spec.ts` | **À faire** (rendu de `/progress` seulement vérifié) |
| Basse | Confirmation de la suppression en masse des orphelins | `series/orphans` | `admin/archives-orphans.spec.ts` | **Non couvert volontairement** : supprimerait les fixtures des workers parallèles |
| Bloqué | Conversion CBR → CBZ | `books/conversion` | `catalog/books-convert.spec.ts` | **Bloqué** : aucun fixture CBR valide ni outillage RAR |

## Conventions

- **Tests autonomes et parallèles** (`fullyParallel: true`) : noms uniques via `uniqueName()`,
  aucune dépendance à l'ordre des fichiers.
- **Nettoyage systématique** : les ressources créées (listes, tokens, users, séries de
  métadonnées) sont supprimées via `support/api.ts` dans un `finally`, même en cas d'échec.
  La bibliothèque `e2e-metadata` (et les racines dédiées `e2e-books` / `e2e-jobs`) est recréée par
  le setup et supprimée par le teardown global.
- **Skips explicites** : `test.skip(...)` uniquement pour une fonctionnalité désactivée
  (`downloads_enabled`, AllDebrid non configuré) ou une donnée absente (livres, jobs,
  bibliothèques). Ne jamais masquer une régression.
- **Pas d'écriture de réglages globaux** (langue, feature flags) : cela rendrait les workers
  dépendants entre eux.
- **Sélecteurs** : privilégier rôles et libellés accessibles ; scoper les requêtes ambiguës
  (`main`, tables, `modal-panel`, `nav[aria-label="Settings sections"]`).

## Données de test déterministes

La suite n'est **pas exécutée en CI** : elle se lance en local, quand on en a besoin. Elle utilise
par défaut les données déjà indexées de la stack de dev, sauf pour les auteurs et les genres :

- **Bibliothèques `e2e-*`** (toujours) : le setup recrée trois racines temporaires et le
  teardown global les supprime. `e2e-metadata` ne contient que des séries éphémères avec des
  auteurs/genres uniques (`support/metadata.ts`) ; `e2e-books` reçoit les livres et séries jetables
  des actions livre/lecture et `e2e-jobs` reste vide pour les rebuilds. La séparation évite qu'un scan supprime les
  séries orphelines créées en parallèle par une autre spec. Les séries de test sont vidées de leurs
  métadonnées avant suppression, et les archives résiduelles sont purgées en recréant les séries
  avant de supprimer chaque bibliothèque.
  Le répertoire est choisi automatiquement (`/tmp`, puis `/var/tmp`, puis `/dev/shm`), avec repli
  sur le répertoire parent si l'API (Docker) ne voit pas le sous-répertoire créé sur l'hôte.
  `E2E_METADATA_LIBRARY_ROOT` force un chemin unique visible par l'API.
- **Seed catalogue** (`E2E_SEED_LIBRARY=1`, optionnel) : copie les CBZ de `fixtures/library/` dans
  un répertoire temporaire, enregistre la bibliothèque `e2e-library` via l'API et déclenche un
  scan. Utile pour un run reproductible sur une base vide.

```bash
# API (7080) native + PostgreSQL via Docker, catalogue déjà indexé
npx playwright test

# Base vide : seed du catalogue en plus des fixtures de métadonnées
E2E_SEED_LIBRARY=1 npx playwright test

# Forcer le répertoire de la bibliothèque de métadonnées
E2E_METADATA_LIBRARY_ROOT=/tmp/mon-e2e npx playwright test
```

Le seed `e2e-library` suppose que l'API et l'indexer tournent **nativement sur le même hôte**
(pas dans Docker), sinon ils ne voient pas les fichiers des fixtures. Sans `E2E_LIBRARY_ROOT`, le
répertoire par défaut est `$TMPDIR/stripstream-e2e-library`. Le setup attend que l'indexer ait
produit tous les livres avant de démarrer les specs.
