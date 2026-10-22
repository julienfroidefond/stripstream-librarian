# Politique de mises à jour (Dependabot)

Renovate a été retiré (`renovate.json` supprimé, app Mend à désinstaller) au profit de
Dependabot (`.github/dependabot.yml`). Politique, alignée sur l'ancienne config Renovate :

- **majeurs** : jamais groupés → une PR par paquet, à traiter manuellement (branches
  `chore/major-*`) ;
- **mineurs pré-1.0** (`0.x → 0.y`, Cargo et npm) : **hors groupe** eux aussi → une PR
  individuelle par bump. Un `0.x → 0.y` casse l'API (Cargo traite le mineur d'une 0.x comme un
  majeur), il ne doit donc jamais être noyé dans un lot de patchs. La liste des dépendances
  directes concernées vit dans les `exclude-patterns` des groupes de
  `.github/dependabot.yml` : **ajouter toute nouvelle dépendance directe en `0.x`** (Cargo,
  npm). Les patchs pré-1.0 restent dans un lot ;
- **`grammers-client` / `grammers-session`** : exclus (`ignore`), migration différée ;
- **autres non-majeurs** : groupés par écosystème (`rust-non-major`, `backoffice-non-major`,
  …) ;
- écosystèmes surveillés : `cargo`, `npm` (backoffice, docs), `github-actions`,
  `docker` (les 3 Dockerfiles), `docker-compose` (`postgres`). Le `package.json` de la racine
  a été supprimé : il n'était consommé par aucun workflow ni Dockerfile (postcss/tailwind y
  doublonnaient ceux du backoffice).

Les Dependabot alerts et les security updates sont activés côté repo (PR de sécurité
automatiques). Dependabot n'a pas de Dependency Dashboard : pour relancer une PR, commenter
`@dependabot rebase` (ou `recreate`), et « Check for updates » dans l'onglet Dependabot pour
une passe manuelle.

## Auto-merge

`.github/workflows/dependabot-automerge.yml` active l'auto-merge GitHub sur les PR Dependabot
qui respectent la politique ci-dessus : patchs et mineurs uniquement, **jamais** un
`0.x → 0.y` (cassant par convention Cargo/npm, donc traité comme un majeur). Un majeur reste
ouvert pour revue manuelle, comme n'importe quelle PR : le workflow ne l'active pas.

Deux prérequis, sans lesquels l'auto-merge ne protège rien :

- les checks `Rust (fmt · clippy · test)`, `Backoffice (tsc · lint · tests)` et `Docs (build)`
  sont **requis** sur la branche protégée. `ci.yml` et `ci-web.yml` se déclenchent donc sur
  *toutes* les PR, avec le filtre de chemins déplacé dans un job interne : un workflow non
  déclenché laisserait le check en « Expected » et bloquerait la PR, alors qu'un job skippé
  par `if:` compte comme un succès ;
- le secret `AUTOMERGE_TOKEN` (PAT fine-grained, `Contents: write` + `Pull requests: write`)
  est indispensable : un merge effectué avec le `GITHUB_TOKEN` ne déclenche pas `deploy.yml`,
  donc `main` avancerait sans bump de version ni déploiement.

Le merge est un squash. Il est attribué au propriétaire du PAT, exactement comme un merge
manuel, ce qui déclenche `deploy.yml` normalement.
