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
| `helpers.ts` | `waitForHydration`, `openSeriesDetail`, `uniqueName`, `escapeRegExp`. |
| `support/api.ts` | Appels directs à l'API Rust avec le token bootstrap : feature flags, nettoyage listes/tokens/users, CRUD bibliothèques/séries, liens et métadonnées de série, notes, jobs, archives et progression de lecture (`X-As-User`). |
| `support/library.ts` | Seed local optionnel (`E2E_SEED_LIBRARY=1`) : copie les CBZ de `fixtures/library/`, crée la bibliothèque via l'API, déclenche un scan et attend l'indexation. |
| `support/metadata.ts` | Bibliothèques temporaires `e2e-metadata`, `e2e-books` et `e2e-jobs` : séries de test avec auteurs/genres uniques, nettoyage, purge des archives et helper `openMetadataSeries` (liste → fiche, avec réchauffement du cache des bibliothèques). |
| `support/books.ts` | Livre ou série jetable multi-tomes : copie un ou plusieurs CBZ dans `e2e-books`, déclenche un scan et attend que l'indexer expose les lignes via `GET /books`. |
| `support/jobs.ts` | Helpers de jobs : sélection de bibliothèque, attente d'un statut terminal et lancement depuis la palette d'actions. |
| `global-teardown.ts` | Suppression des bibliothèques temporaires et de leurs archives en fin de run. |
| `fixtures/library/` | Mini-bibliothèque déterministe (série `E2E Series`, tomes 1 et 2) utilisée par le seed local et par les livres jetables. |
| `tests/e2e/.auth/state.json` | Session admin générée par le setup (ignoré par git). |

L'interface est en **anglais** par défaut ; seules les libellés de la page de connexion sont codés
en français. Le cookie `as_user_id` active les UI « utilisateur actif » sans changer la session
admin.

## Matrice des domaines

| Domaine | Dossier | Couvert | Non couvert (voir backlog) |
|---------|---------|---------|---------------------------|
| Shell / navigation | `shell/` | Rendu des 15 routes, menus desktop et tiroir mobile, liens directs, thème clair/sombre, pastille jobs, redirection auth, login invalide/valide, logout, modales (Escape, bouton, validation, focus trap et restauration), erreur d'enregistrement, filtres mobiles. | Bascule de langue. |
| Dashboard | `dashboard/` | Cartes de statistiques, répartitions, sélecteurs de période, légendes de graphiques, liens des cartes rapides, lectures en cours/récentes et filtre lecteur, lien téléchargement récent (si activé). | — |
| Catalogue | `catalog/` | Recherche séries (`?q=`), édition de tags, modale métadonnées, confirmation de suppression annulée, **renommage réel + suppression via Actions**, **édition de champs + verrouillage/déverrouillage**, **fusion de deux séries avec déplacement du lien de métadonnées**, **notation persistée puis retirée**, **actions livre**, **contrôles des listes** (filtres, tris, pagination, bascule grille/liste, onglets sur séries/livres/auteurs/métadonnées), **création manuelle de série**, **atelier genres** (tagging, sélection, suggestion IA, filtre bibliothèque, Include/Exclude, reset), **actions de carte** (refresh ciblé, mark-read), **renommage des fichiers par templates** (aperçu, type/tome), **séries liées et recommandations**. | Conversion CBR → CBZ. |
| Métadonnées | `metadata/` | Onglets Séries/Livres, filtre par gap, compteur, recherche vide, **promotion d'un lien approuvé et synchronisation depuis `metadata_json`**, **recherche provider mockée : approbation, synchronisation « série seule » puis déliaison**, **verrouillage d'un champ préservé pendant une synchronisation mockée**. | Providers externes réels ; volumes manquants. |
| Lecture | `reading/` | Progression de l'utilisateur actif (vide ou peuplée), **lu/non lu persistant + restauration**, **série entière + statut par tome**, cycle complet des listes (création, ajout/retrait, suppression, renommage, réordonnancement, groupement), **entrées carte/fiche + badge série vide**, **restrictions de genres**, **tri automatique**, **progression admin par lecteur sur la fiche série**. | Suggestions IA et provider de statut de lecture externes. |
| Bibliothèques | `libraries/` | Cartes, formulaire d'ajout, navigation vers les séries et livres, **réglages persistés (scan, watcher, providers, tags) puis restaurés via l'API**. | Création/suppression réelle de bibliothèque (volume hôte). |
| Acquisition | `acquisition/` | Drapeau `downloads_enabled`, onglets Trending/Wishlist, panneau des séries masquées, série sans livre, **actions des onglets** (provider/période, hard refresh, hide/unhide, ajout via modale), **filtres, tri, recherche et pagination Prowlarr mockés**. | Prowlarr/Telegram/DDL et téléchargements réels. |
| Opérations | `operations/` | Jobs (filtres, détail, palette/recherche/Échap), **rebuild ciblé + replay**, **palette (rescan, rebuild complet, batch metadata, confirmations)**, **miniatures (génération + régénération)**, diagnostics (indicateurs, refresh, purge cache, **sonde mockée, copie du rapport, toast d'erreur**). | Annulation de job, actions de jobs liées aux intégrations externes, téléchargements réels. |
| Admin | `admin/` | Matrice des onglets Paramètres, tests de connexion mockés (Prowlarr, qBittorrent, Telegram, DDL, AllDebrid si configuré), CRUD token/user, réassignation + renommage inline, bascule utilisateur actif + restrictions de genres, **orphelins/archives (archive, suppression unitaire, annulation en masse)**, **actions locales des settings (cache, format de renommage, IA, mappings, AniList, Komga, Telegram, providers de métadonnées, moteur et providers DDL) avec écritures mockées**. | Écritures réelles de réglages globaux/secrets, confirmation de la suppression en masse des orphelins, connexions externes réelles. |

## Backlog — blocages et exclusions

L'audit des parcours documentés et des actions UI est soldé : tout ce qui est exerçable sans effet
externe est couvert dans la matrice ci-dessus. Ne restent ici que les blocages techniques et les
exclusions volontaires ; aucun parcours déjà couvert n'y figure.

### Bloqués

| Parcours | Doc | Spec prévue | Raison |
|----------|-----|-------------|--------|
| Volumes manquants et bascule d'affichage | `series/missing-volumes` | `metadata/missing-volumes.spec.ts` | Aucune API publique ne seed `external_book_metadata` |
| Conversion CBR → CBZ | `books/conversion` | `catalog/books-convert.spec.ts` | Aucun fixture CBR valide ni outillage RAR |
| Annulation d'un job en cours | `jobs/overview` | `operations/jobs-cancel.spec.ts` | Nécessite un job long et contrôlable ; les jobs de test se terminent trop vite |

### Exclusions volontaires

| Parcours | Doc | Spec | Raison |
|----------|-----|------|--------|
| Suppression en masse des orphelins (confirmation) | `series/orphans` | `admin/archives-orphans.spec.ts` | Supprimerait les fixtures des workers parallèles |
| Création/suppression réelle d'une bibliothèque via l'UI | `libraries/management` | `libraries/libraries.spec.ts` | Effet destructif sur un volume hôte partagé ; le cycle est couvert par l'API du setup et du teardown |
| Téléchargements et intégrations réelles (Prowlarr, Telegram, AniList, Komga, DDL, AllDebrid) | `downloads/*`, `integrations/*`, `jobs/telechargements`, `jobs/telegram`, `jobs/anilist` | specs d'intégration concernées | Services externes, secrets et effets de bord non réversibles ; les tests de connexion et les actions visibles sont mockés |
| Actions de la palette de jobs liées aux intégrations | `jobs/metadonnees`, `jobs/telechargements`, `jobs/telegram`, `jobs/anilist` | `operations/jobs-actions.spec.ts` | Metadata rematch/refresh, reading status match/push, rating pull, download detection, RSS/Prowlarr, DDL/Telegram : appels réels non mockables depuis le navigateur |
| Écritures réelles de réglages globaux (langue, feature flags, limites, images, cache, miniatures, clients, moteur DDL, secrets) | `getting-started/settings`, `getting-started/configuration` | `admin/settings-actions.spec.ts`, `admin/connection-test.spec.ts` | Modifieraient la stack partagée ; les actions locales sont couvertes avec des `route.fulfill` qui interceptent les écritures |
| Règles et contrats backend | `reference/api`, `reference/environment`, `reference/jobs`, `reference/users-tokens` | — | Documentation de contrat, sans action UI ; couverte par les tests Rust et unitaires |

## Conventions

- **Tests autonomes et parallèles** (`fullyParallel: true`) : noms uniques via `uniqueName()`,
  aucune dépendance à l'ordre des fichiers. Après une navigation, attendre `waitForHydration(page)`
  (ou `waitForHydration(page, "input")` sur les pages de réglages, qui n'ont pas de `<form>`)
  avant d'interagir.
- **Nettoyage systématique** : les ressources créées (listes, tokens, users, séries de
  métadonnées, livres/séries jetables, jobs, progression de lecture) sont supprimées via
  `support/api.ts` ou `support/books.ts` dans un `finally`, même en cas d'échec.
  La bibliothèque `e2e-metadata` (et les racines dédiées `e2e-books` / `e2e-jobs`) est recréée par
  le setup et supprimée par le teardown global.
- **Skips explicites** : `test.skip(...)` uniquement pour une fonctionnalité désactivée
  (`downloads_enabled`, AllDebrid non configuré) ou une donnée absente (livres, jobs,
  bibliothèques). Ne jamais masquer une régression.
- **Pas d'écriture de réglages globaux** (langue, feature flags, seuils) : les tests d'actions de
  réglages interceptent les POST avec `page.route` et ne persistent rien, afin que les workers
  restent indépendants.
- **Sélecteurs** : privilégier rôles et libellés accessibles ; scoper les requêtes ambiguës
  (`main`, tables, `modal-panel`, `nav[aria-label="Settings sections"]`).

## Données de test déterministes

La suite n'est **pas exécutée en CI** : elle se lance en local, quand on en a besoin. Elle utilise
par défaut les données déjà indexées de la stack de dev, sauf pour les auteurs, genres,
livres/séries jetables, jobs et progression de lecture :

- **Bibliothèques `e2e-*`** (toujours) : le setup recrée trois racines temporaires et le
  teardown global les supprime. `e2e-metadata` ne contient que des séries éphémères avec des
  auteurs/genres uniques (`support/metadata.ts`) ; `e2e-books` reçoit les livres et séries jetables
  des actions livre/lecture, du renommage, des recommandations et du dashboard ; `e2e-jobs` reste
  vide pour les rebuilds. La séparation évite qu'un scan supprime les séries orphelines créées en
  parallèle par une autre spec. Les séries de test sont vidées de leurs métadonnées avant
  suppression, et les archives résiduelles sont purgées en recréant les séries avant de supprimer
  chaque bibliothèque.
  Le répertoire est choisi automatiquement (`/tmp`, puis `/var/tmp`, puis `/dev/shm`), avec repli
  sur le répertoire parent si l'API (Docker) ne voit pas le sous-répertoire créé sur l'hôte.
  `E2E_METADATA_LIBRARY_ROOT` force un chemin unique visible par l'API.
- **Progression et dashboard** : les tests de lecture écrivent la progression via le proxy Next
  (`PATCH /api/books/:id/progress`). Ce handler invalide le tag `stats` côté Next, et l'API Rust
  purge son cache mémoire `/stats` (20 s) à chaque mutation de progression. Les listes
  « Currently reading » / « Recently read » reflètent donc les fixtures immédiatement, sans
  attente de cache.
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
