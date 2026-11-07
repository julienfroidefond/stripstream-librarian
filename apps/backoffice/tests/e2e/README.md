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
| `auth.setup.ts` | Projet `setup` : connexion admin, création/réutilisation du lecteur `e2e-reader`, nettoyage des ressources `e2e-*` orphelines, sauvegarde de `storageState` + cookie `as_user_id`. |
| `env.ts` | Lecture de `.env.local` (identifiants admin). |
| `helpers.ts` | `openFirstSeries`, `uniqueName`, `escapeRegExp`. |
| `support/api.ts` | Appels directs à l'API Rust avec le token bootstrap : feature flags, nettoyage listes/tokens/users. |
| `support/library.ts` | Seed local optionnel (`E2E_SEED_LIBRARY=1`) : copie les CBZ de `fixtures/library/`, crée la bibliothèque via l'API, déclenche un scan et attend l'indexation. |
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
| Catalogue | `catalog/` | Recherche séries (`?q=`), édition de tags (ajout/retrait sans sauvegarde), modale métadonnées, confirmation de suppression annulée, lecture (bouton, volumes), liste/détail livre + infos techniques, recherche vide, auteurs (liste/fiche si données), genres (stats/filtres). | Soumission des formulaires d'édition et suppression réelle (destructif/global). |
| Métadonnées | `metadata/` | Onglets Séries/Livres, filtre par gap, compteur de résultats, recherche sans résultat. | Synchronisation réelle vers un provider externe. |
| Lecture | `reading/` | Progression de l'utilisateur actif (vide ou peuplée), cycle complet liste de lecture : création, ajout/retrait de série, suppression (dialogue natif), nettoyage API. | Renommage de liste, suggestions IA, provider de statut de lecture. |
| Bibliothèques | `libraries/` | Cartes, formulaire d'ajout, navigation vers les séries et les livres d'une bibliothèque. | Création/suppression réelle de bibliothèque (volume hôte). |
| Acquisition | `acquisition/` | Drapeau `downloads_enabled` (page ou redirection), découverte : onglets Trending/Wishlist, panneau des séries masquées. | Prowlarr/Telegram/DDL et téléchargements réels (configuration externe). |
| Opérations | `operations/` | Jobs : filtres, détail, palette d'actions (recherche, Échap). Diagnostics : indicateurs, sections, rafraîchissement. | Lancement/replay de job, purge de cache (écritures). |
| Admin | `admin/` | Matrice des onglets Paramètres, présence des tests de connexion, CRUD token (création → révocation → suppression), CRUD utilisateur lecteur. | Enregistrement de réglages globaux, réassignation de token, restrictions de genres, archives/orphelins peuplés. |

## Conventions

- **Tests autonomes et parallèles** (`fullyParallel: true`) : noms uniques via `uniqueName()`,
  aucune dépendance à l'ordre des fichiers.
- **Nettoyage systématique** : les ressources créées (listes, tokens, users) sont supprimées via
  `support/api.ts` dans un `finally`, même en cas d'échec.
- **Skips explicites** : `test.skip(...)` uniquement pour une fonctionnalité désactivée
  (`downloads_enabled`) ou une donnée absente (auteurs, jobs). Ne jamais masquer une régression.
- **Pas d'écriture de réglages globaux** (langue, feature flags) : cela rendrait les workers
  dépendants entre eux.
- **Sélecteurs** : privilégier rôles et libellés accessibles ; scoper les requêtes ambiguës
  (`main`, tables, `modal-panel`, `nav[aria-label="Settings sections"]`).

## Données de test déterministes (optionnel)

La suite n'est **pas exécutée en CI** : elle se lance en local, quand on en a besoin. Par défaut,
elle utilise les données déjà indexées de la stack de dev. Pour un run reproductible sur une base
vide, ou pour tester le catalogue sans dépendre des données réelles, `E2E_SEED_LIBRARY=1` copie
les CBZ de `fixtures/library/` dans un répertoire temporaire, enregistre la bibliothèque
`e2e-library` via l'API et déclenche un scan.

Le seed suppose que l'API et l'indexer tournent **nativement sur le même hôte** (pas dans Docker),
sinon ils ne voient pas le répertoire des fixtures :

```bash
# API (7080) et indexer (7081) lancés nativement, PostgreSQL via Docker
E2E_SEED_LIBRARY=1 E2E_LIBRARY_ROOT=/tmp/stripstream-e2e-library npx playwright test
```

Sans `E2E_LIBRARY_ROOT`, le répertoire par défaut est `$TMPDIR/stripstream-e2e-library`. Le setup
attend que l'indexer ait produit tous les livres des fixtures avant de démarrer les specs. Le seed
reste désactivé par défaut.

## Prochaines étapes

1. Ajouter un jeu de données de test (auteurs, archives, orphelins) pour supprimer les skips.
2. Couvrir les parcours d'écriture métier : édition/suppression de série, synchronisation
   métadonnées, lancement de job, test de connexion téléchargement.
3. Tester le changement d'utilisateur actif et les restrictions de genres.
