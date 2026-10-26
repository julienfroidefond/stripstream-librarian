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
- **digests d'images** (`FROM image:tag@sha256:…`, tags non semver du type
  `latest-rust-1-bookworm`) : Dependabot ne renseigne alors **aucun** `updateType`, si bien que
  ces PR tombaient dans la catégorie « inconnu » et restaient ouvertes pour toujours. Elles
  sont éligibles à l'auto-merge **si le diff ne bouge que le digest** : uniquement des lignes
  `image@sha256:` et le même `repo:tag` des deux côtés. Un changement de tag sous un tag
  flottant (`latest-rust-1-bookworm` → `latest-rust-1-trixie`) reste manuel ;
- écosystèmes surveillés : `cargo`, `npm` (backoffice, docs), `github-actions`,
  `docker` (les 3 Dockerfiles), `docker-compose` (`postgres`). Le `package.json` de la racine
  a été supprimé : il n'était consommé par aucun workflow ni Dockerfile (postcss/tailwind y
  doublonnaient ceux du backoffice).

Les Dependabot alerts et les security updates sont activés côté repo (PR de sécurité
automatiques). Dependabot n'a pas de Dependency Dashboard : pour relancer une PR, commenter
`@dependabot rebase` (ou `recreate`), et « Check for updates » dans l'onglet Dependabot pour
une passe manuelle.

## Auto-merge

`.github/workflows/dependabot-automerge.yml` merge automatiquement les PR Dependabot qui
respectent la politique ci-dessus : patchs et mineurs uniquement, **jamais** un
`0.x → 0.y` (cassant par convention Cargo/npm, donc traité comme un majeur), et les bumps de
digest d'image quand le diff ne change que le digest. Un majeur reste ouvert pour revue
manuelle, comme n'importe quelle PR : le workflow ne le touche pas.

La décision vit dans `.github/scripts/dependabot-eligibility.jq`, testé par
`.github/scripts/dependabot-eligibility.test.sh` (job `Dependabot policy` de `ci.yml`) : un
filtre jq faux soit bloquerait tous les bumps, soit mergerait un cassant.

Deux prérequis, sans lesquels l'automatisation ne protège rien :

- les checks `Rust (fmt · clippy · test)`, `Backoffice (tsc · lint · tests)` et `Docs (build)`
  sont **requis** sur la branche protégée. `ci.yml` et `ci-web.yml` se déclenchent donc sur
  *toutes* les PR, avec le filtre de chemins déplacé dans un job interne : un workflow non
  déclenché laisserait le check en « Expected » et bloquerait la PR, alors qu'un job skippé
  par `if:` compte comme un succès ;
- le secret `AUTOMERGE_TOKEN` (PAT fine-grained, `Contents: write` + `Pull requests: write`)
  est indispensable : un merge effectué avec le `GITHUB_TOKEN` ne déclenche pas `deploy.yml`,
  donc `main` avancerait sans bump de version ni déploiement.

Le workflow n'utilise pas l'auto-merge natif de GitHub : la mutation
`enablePullRequestAutoMerge` répond « Resource not accessible by personal access token » pour un
PAT fine-grained, quelles que soient ses permissions sur le repo (vérifié avec `admin: true`).
Il attend donc lui-même les trois checks requis, approuve la PR — le ruleset exige une
approbation supplémentaire pour un push non attribué, et Dependabot en est un — puis lance
`gh pr merge --squash --delete-branch`, jusqu'à 60 minutes. Un nouveau push sur la PR relance
l'attente.

Le merge est un squash attribué au propriétaire du PAT, exactement comme un merge manuel, ce
qui déclenche `deploy.yml` normalement.
