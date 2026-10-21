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
- écosystèmes surveillés : `cargo`, `npm` (backoffice, docs, racine), `github-actions`,
  `docker` (les 3 Dockerfiles), `docker-compose` (`postgres`).

Les Dependabot alerts et les security updates sont activés côté repo (PR de sécurité
automatiques). Dependabot n'a pas de Dependency Dashboard : pour relancer une PR, commenter
`@dependabot rebase` (ou `recreate`), et « Check for updates » dans l'onglet Dependabot pour
une passe manuelle.
