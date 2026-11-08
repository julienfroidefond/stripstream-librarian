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
| `auth.setup.ts` | Projet `setup` : connexion admin, création/réutilisation du lecteur `e2e-reader`, nettoyage des ressources `e2e-*` orphelines, préparation de la bibliothèque temporaire de métadonnées, sauvegarde de `storageState` + cookie `as_user_id`. |
| `env.ts` | Lecture de `.env.local` (identifiants admin). |
| `helpers.ts` | `openFirstSeries`, `uniqueName`, `escapeRegExp`. |
| `support/api.ts` | Appels directs à l'API Rust avec le token bootstrap : feature flags, nettoyage listes/tokens/users, CRUD bibliothèques/séries, liens et métadonnées de série, jobs, archives et progression de lecture (`X-As-User`). |
| `support/library.ts` | Seed local optionnel (`E2E_SEED_LIBRARY=1`) : copie les CBZ de `fixtures/library/`, crée la bibliothèque via l'API, déclenche un scan et attend l'indexation. |
| `support/metadata.ts` | Bibliothèque temporaire `e2e-metadata` : séries de test avec auteurs/genres uniques, nettoyage et purge des archives. |
| `global-teardown.ts` | Suppression de la bibliothèque temporaire et de ses archives en fin de run. |
| `fixtures/library/` | Mini-bibliothèque déterministe (série `E2E Series`, tomes 1 et 2) utilisée par le seed local. |
| `tests/e2e/.auth/state.json` | Session admin générée par le setup (ignoré par git). |

L'interface est en **anglais** par défaut ; seules les libellés de la page de connexion sont codés
en français. Le cookie `as_user_id` active les UI « utilisateur actif » sans changer la session
admin.

## Matrice des domaines

| Domaine | Dossier | Couvert | Non couvert volontairement |
|---------|---------|---------|----------------------------|
| Shell / navigation | `shell/` | Rendu des 15 routes, menus desktop et tiroir mobile, liens directs, thème clair/sombre, pastille jobs, redirection auth, login invalide/valide, logout, modale générique (Escape / bouton). | Bascule de langue (réglage global persistant). |
| Dashboard | `dashboard/` | Cartes de statistiques, répartitions, sélecteurs de période des graphiques. | — |
| Catalogue | `catalog/` | Recherche séries (`?q=`), édition de tags (ajout/retrait sans sauvegarde), modale métadonnées, confirmation de suppression annulée, **renommage réel + suppression via Actions et vérification de l'archive avec auteurs/genres**, lecture (bouton, volumes), liste/détail livre + infos techniques, recherche vide, auteurs (recherche + fiche via fixture), genres (stats/filtres + renommage réel via fixture). | Synchronisation via appels réseau aux providers externes. |
| Métadonnées | `metadata/` | Onglets Séries/Livres, filtre par gap, compteur de résultats, recherche sans résultat, **promotion d'un lien approuvé et synchronisation des auteurs/genres/description depuis `metadata_json`**. | Recherche et rafraîchissement via un provider externe réel. |
| Lecture | `reading/` | Progression de l'utilisateur actif (vide ou peuplée), **marquage lu/non lu avec persistance après rechargement et restauration de l'état d'origine via l'API**, cycle complet liste de lecture : création, ajout/retrait de série, suppression (dialogue natif), nettoyage API. | Renommage de liste, suggestions IA, provider de statut de lecture. |
| Bibliothèques | `libraries/` | Cartes, formulaire d'ajout, navigation vers les séries et les livres d'une bibliothèque. | Création/suppression réelle de bibliothèque (volume hôte). |
| Acquisition | `acquisition/` | Drapeau `downloads_enabled` (page ou redirection), découverte : onglets Trending/Wishlist, panneau des séries masquées. | Prowlarr/Telegram/DDL et téléchargements réels (configuration externe). |
| Opérations | `operations/` | Jobs : filtres, détail, palette d'actions (recherche, Échap), **lancement d'un rebuild ciblé puis replay avec vérification des statuts terminaux**. Diagnostics : indicateurs, sections, rafraîchissement. | Annulation de job, purge de cache, téléchargements réels. |
| Admin | `admin/` | Matrice des onglets Paramètres, présence des tests de connexion, **test AllDebrid mocké avec rendu du résultat**, CRUD token (création → révocation → suppression), CRUD utilisateur lecteur, **bascule d'utilisateur actif + restrictions de genres (série masquée puis réaffichée pour Admin)**, **fixtures déterministes Orphan series → Archives**. | Enregistrement de réglages globaux, réassignation de token, connexions externes réelles. |

## Backlog — parcours documentés non couverts

Recensement issu de `apps/docs/`. Ces parcours ne sont pas des exclusions volontaires : ils
correspondent à des actions visibles du backoffice qui n'ont pas encore de test Playwright. Les
providers externes réels, les réglages globaux et les règles métier pures (matching ISBN,
restauration d'archives, internes de conversion) restent hors périmètre E2E.

| Priorité | Parcours | Doc | Spec cible | Statut |
|----------|----------|-----|------------|--------|
| Haute | Actions livre : édition, téléchargement, renommage, suppression (dernier livre → série archivée), conversion CBR → CBZ | `books/*` | `catalog/book-write.spec.ts` + fixture livre jetable | À faire |
| Haute | Édition et verrouillage des champs d'une série | `series/management`, `metadata/sync` | `catalog/series-write.spec.ts` | À faire |
| Haute | Fusion de deux séries | `series/management` | `catalog/series-merge.spec.ts` | À faire |
| Haute | Volumes manquants et bascule d'affichage | `series/missing-volumes` | `metadata/missing-volumes.spec.ts` | À faire |
| Haute | Notation d'une série par le lecteur actif | `series/ratings` | `catalog/ratings.spec.ts` | À faire |
| Haute | Réglages d'une bibliothèque (scan, watcher, providers, tags) | `libraries/management` | `libraries/settings-write.spec.ts` | À faire |
| Haute | Recherche, approbation et rejet de métadonnées | `metadata/sync`, `metadata/providers` | `metadata/search.spec.ts` | À faire |
| Quick win | Réassignation d'un token et renommage inline d'un utilisateur | `reference/users-tokens` | `admin/tokens.spec.ts` | À faire |
| Quick win | Suppression unitaire et par page des séries orphelines | `series/orphans` | `admin/archives-orphans.spec.ts` | À faire |
| Quick win | Purge du cache depuis les diagnostics | `getting-started/diagnostics` | `operations/diagnostics.spec.ts` | À faire |
| Quick win | Tests de connexion Prowlarr, qBittorrent, Telegram et DDL mockés | `integrations/*`, `downloads/*` | `admin/connection-test.spec.ts` | À faire |
| Quick win | Renommage, réordonnancement et groupement des listes de lecture | `series/reading-lists` | `reading/lists.spec.ts` | À faire |
| Quick win | Wishlist : série à zéro livre et filtre par bibliothèque | `discovery/wishlist` | `acquisition/discovery.spec.ts` | À faire |
| Après fixture livre | Séries liées et recommandations | `series/related` | `catalog/series-related.spec.ts` | À faire |
| Après fixture livre | Vue admin de progression multi-utilisateurs | `series/reading-progress` | `reading/progress-admin.spec.ts` | À faire |

## Conventions

- **Tests autonomes et parallèles** (`fullyParallel: true`) : noms uniques via `uniqueName()`,
  aucune dépendance à l'ordre des fichiers.
- **Nettoyage systématique** : les ressources créées (listes, tokens, users, séries de
  métadonnées) sont supprimées via `support/api.ts` dans un `finally`, même en cas d'échec.
  La bibliothèque `e2e-metadata` est recréée par le setup et supprimée par le teardown global.
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

- **Bibliothèque `e2e-metadata`** (toujours) : le setup recrée une bibliothèque temporaire et le
  teardown global la supprime. Les specs y créent des séries éphémères avec des auteurs/genres
  uniques (`support/metadata.ts`), puis vident leurs métadonnées avant suppression. Les archives
  résiduelles sont purgées en recréant les séries avant de supprimer la bibliothèque.
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
