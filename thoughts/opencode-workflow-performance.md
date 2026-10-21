# Audit — performance du workflow OpenCode (issue #151)

- **Date** : 2026-10-21
- **Référence** : branche `julienfroidefond/issue-151-opencode-workflow`
- **Périmètre** : `.github/workflows/opencode.yml`, `.opencode/`, `.agents/skills/`, `AGENTS.md` /
  `CLAUDE.md` (racine + modules), `package.json` racine
- **Objet** : expliquer où passe le temps, appliquer les optimisations sûres et chiffrées, lister
  les choix restants (décisions produit, non implémentées).

---

## 1. Résumé

| Levier | Avant | Après | Gain mesuré |
|---|---|---|---|
| Cache cargo (CI) | jamais sauvegardé (`cache-mode 'read'`) | sauvegardé et partagé entre jobs | **≈ 7 min de `clippy` à froid + ≈ 3 min de build de test** par run |
| Prompt du 1er tour | 15 462 tokens | **12 292 tokens** | **−3 170 (−20,5 %)** |
| Contexte possédé par le repo | 7 204 tokens (46,6 %) | **4 034 tokens (32,8 %)** | **−3 170 (−44 %)** |
| `AGENTS.md` racine | 6 114 tokens | **3 143 tokens** | **−2 971 (−48,6 %)** |
| `apps/backoffice/CLAUDE.md` | 1 334 tokens | **469 tokens** | **−865 (−65 %)** |
| `nextest` dans le job `comment` | absent (repli `cargo test`) | installé | parité de validation avec `issue` |

Le temps du workflow est à **98 % du temps modèle** : sur le run
[39477381484](https://github.com/julienfroidefond/stripstream-librarian/actions/runs/39477381484)
(51 min 00 s), l'installation ne prend que **42 s** (18:44:29 → 18:45:11) et la boucle agent occupe
**50 min 09 s** (18:45:11 → 19:35:20). Optimiser l'image, le checkout ou le parsing de config ne sert
donc à rien : les deux seuls leviers réels sont **le cache de compilation** et **le contexte injecté
à chaque pas**.

---

## 2. Méthode

1. **Logs CI** : `gh run view <id> --log`, 10 runs non `skipped`, analyse des horodatages par pas
   (`step: N`) et des appels d'outils. Run de référence : 39477381484 (issue #? → PR, 51 min).
2. **Prompt réel** : `opencode run --format json "Reply with exactly: PONG"` puis
   `opencode session export <sid>` ; le prompt total est `tokens.input + tokens.cache.read`
   (validé par la linéarité : doubler `AGENTS.md` ajoutait exactement 2 × 6 114 tokens).
3. **Comptage exact** des fichiers d'instructions avec `tiktoken` (`cl100k_base`), via
   `uv run --quiet --with tiktoken python`.
4. **Témoin** : même mesure dans un dépôt git vide → **8 258 tokens** (prompt système + outils +
   skills globales). La part « repo » est la différence.

---

## 3. Diagnostic avant

### 3.1 Durées

| Run | Événement | Durée |
|---|---|---|
| 39477381484 | issue | **51 min** |
| 39458124031 | issue | **56 min** |
| 39443375938 | issue | 60 min (annulé par le timeout) |
| 39381189511 | issue | 30 min (annulé) |
| 39277290099 | issue | 77 min (annulé) |

Toutes les tâches utiles se déroulent dans « Run OpenCode ».

### 3.2 Décomposition du run de 51 min (148 pas de modèle)

| Phase | Fenêtre | Durée | Pas |
|---|---|---|---|
| Setup (checkout, toolchain, cache, nextest, pdfium, Postgres) | 18:44:29 → 18:45:11 | 42 s | — |
| Agent principal | 18:45:11 → 19:35:20 | 50,1 min | 80 |
| dont `cargo clippy --workspace --all-targets` **à froid** | 18:55:57 → 19:03:43 | **7 min 43 s** | 1 |
| dont `cargo nextest run --workspace` (build des binaires de test) | 19:03:43 → ≤ 19:06:53 | **≈ 3 min** | 1 |
| dont sous-agent `reviewer` | 19:06:57 → 19:32:36 | **25,7 min** | **68** |

Points notables : 148 pas pour une issue, dont **46 % en revue** ; 6 réponses HTTP 503 et 3 erreurs
de stream (reprises) ; 3 commandes shell longues (clippy 465 s, npm `test:unit`, fmt) représentent
l'essentiel des minutes « hors modèle ».

### 3.3 Cause n°1 — le cache cargo n'est jamais écrit puis relu

Le workflow déclare un bloc `permissions:` (`id-token: write`) : toute permission non listée passe à
`none`, donc **`actions: write` est absent** et `Swatinem/rust-cache` ne peut pas sauvegarder.
Preuve dans le log, à chaque run :

```
18:45:05  No cache found.
19:35:28  Cache save skipped: the effective cache-mode 'read' does not permit writes.
```

Vérifié de la même façon sur 39458124031, 39327329112 et 39381189511. `ci.yml`, qui n'a pas de bloc
`permissions:`, sauvegarde normalement — mais ses caches vivent sur les refs de PR et sa clé contient
l'id du job (`v0-rust-rust-…`), donc inutilisables depuis `main`.

Conséquence : **chaque run recompile tout le workspace** avant que l'agent puisse valider quoi que ce
soit, et paie ~11 min de compilation par run, parfois plusieurs fois (le prompt demande `clippy` puis
`nextest`).

### 3.4 Cause n°2 — le contexte d'instructions est volumineux et redondant

Prompt du premier tour : **15 462 tokens** (input 12 262 + cache read 3 200), dont **7 204 (47 %)**
viennent du repo. Détail du fichier racine :

| Section | Tokens | Redondance |
|---|---|---|
| Préambule | 230 | — |
| §1 Build/Lint/Test | 1 489 | politique Dependabot (~480) hors sujet ; narratif CI/deploy (~530) lisible à la demande |
| §2 Code Style | 1 529 | nombreux blocs d'exemple « évidents » |
| §3 Structure | 985 | arborescence ASCII recopiée dans les guides modules |
| §4 Common Patterns | 759 | pipeline indexer + path remapping déjà dans `apps/indexer/AGENTS.md` |
| §5 Gotchas | 1 122 | 8 puces sur 12 déjà couvertes par un guide module |

À cela s'ajoute `apps/backoffice/CLAUDE.md` : **1 334 tokens** de documentation générique de `rtk`
(un outil local, absent des runners CI), sans aucun contenu propre au backoffice.

Coût cumulé : 148 appels de modèle × 3 170 tokens évitables ≈ **469 k tokens de prompt par run**
(pour l'essentiel relus depuis le cache provider, donc surtout de la latence de prefill, pas du
dollar).

---

## 4. Changements appliqués

### 4.1 CI — cache réellement écrit et partagé (`.github/workflows/opencode.yml`)

- `permissions: + actions: write` sur les deux jobs (`issue`, `comment`), avec commentaire
  explicatif : c'est la permission requise par `actions/cache` depuis la restriction des scopes.
- `add-job-id-key: false` sur les deux étapes `rust-cache` : les jobs `issue` et `comment`
  partagent maintenant la même clé de restauration au lieu de maintenir deux caches.
- `taiki-e/install-action@nextest` ajouté au job `comment` : sans lui, un `/oc` « corrige et
  valide » retombait sur `cargo test` (plus lent) alors que le prompt lui demande `cargo nextest`.

Gain attendu : **7 à 8 min par run** (clippy à froid) + **1 à 3 min** (binaires de test), sur un run
de 18 à 51 min. Le premier run après merge réamorcera le cache (une seule fois).

### 4.2 Contexte — `AGENTS.md` dégraissé, contenu déplacé, pas supprimé

| Fichier | Avant | Après | Δ |
|---|---|---|---|
| `AGENTS.md` | 6 114 | **3 143** | **−2 971 (−48,6 %)** |
| `apps/api/AGENTS.md` | 1 500 | 1 716 | +216 |
| `apps/indexer/AGENTS.md` | 2 366 | 2 447 | +81 |
| `apps/backoffice/AGENTS.md` | 2 704 | 2 833 | +129 |
| `apps/backoffice/CLAUDE.md` | 1 334 | **469** | **−865** |
| `.opencode/agent/reviewer.md` | 444 | 489 | +45 |

Principe : la racine ne garde que le **transversal** (commandes, style, structure, patterns croisés,
gotchas globaux) ; le spécifique descend dans le guide du module concerné, qui n'est lu que si
l'agent touche ce module.

- Nouveaux fichiers de référence, lus **à la demande** : `.github/workflows/README.md` (CI/CD,
  images, keychain du runner, cache) et `.github/dependabot-policy.md` (politique de mises à jour,
  reprise telle quelle). La racine pointe vers eux en une ligne.
- Déplacé vers `apps/api/AGENTS.md` : import torrent en mode `replace`, compteurs
  `already_existed`, détection de torrent en double, `DISTINCT ON (volume_number)`.
- Déplacé vers `apps/indexer/AGENTS.md` : propagation de `volume_type` au rescan.
- Déplacé vers `apps/backoffice/AGENTS.md` : `staleTimes`, `revalidate`, `unoptimized`, et le
  gotcha Next.js 16 standalone (`router.replace()` inopérant).
- Supprimé parce que déjà présent dans un guide module (vérifié un par un) : CLI externes,
  `page_count` NULL, génération de miniatures, extraction de série, keychain de déploiement.
- Réduit sans perte : exemples de code illustratifs (imports, transaction, timeout, struct
  `utoipa`), tableau de nommage conservé, tableau des ports conservé, commandes essentielles
  conservées.
- `apps/backoffice/CLAUDE.md` : le dump `rtk` (138 lignes) devient une page compacte (31 lignes) qui
  conserve la règle d'or, la liste des filtres par catégorie et l'avertissement « absent en CI ».

### 4.3 Revue — moins de tokens, mêmes vérifications

`.opencode/agent/reviewer.md` demande désormais `git status` + `git diff --stat origin/main` puis le
diff **fichier par fichier** (`git diff origin/main -- <path>`), au lieu de charger le diff complet
d'un coup. Aucun critère de revue n'est retiré.

### 4.4 Validation alignée sur les filtres de la CI

Le prompt imposait `fmt` + `clippy --workspace` + `nextest --workspace` **à chaque** issue, alors
que `ci.yml` est filtré par chemins (`apps/api`, `apps/indexer`, `crates`, `Cargo.*`,
`infra/migrations`, `rust-toolchain*`) et que `ci-web.yml` ne s'occupe que des apps web. Une issue
purement backoffice validait donc du Rust qu'elle n'avait pas modifié — et que la PR ne fera pas
valider non plus. Le bloc « Validation » est maintenant conditionné par surface :

| Surfaces touchées | Commandes |
|---|---|
| Rust | `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo nextest run --workspace` |
| `apps/backoffice`, `apps/docs` | `npm ci`, `next typegen`, `tsc --noEmit`, `lint`, `test:unit` |
| docs uniquement | aucune build |

Gain : ~2-4 min par issue front-only avec un cache chaud, jusqu'à ~11 min à froid. La protection de
branche est inchangée : une PR front-only ne déclenchait déjà pas `ci.yml`. Le setup du job
(toolchain Rust, nextest, libpdfium, Postgres) reste inconditionnel — 20-40 s, non rentable à
conditionner.

Ce filtrage porte sur **l'exécution** des suites, pas sur l'écriture des tests : la règle
« any code change MUST ship with tests » (AGENTS.md §1) et la consigne « add tests for the nominal
path, the empty/no-op path, … » du prompt restent en vigueur. Une modification Rust écrit et exécute
ses tests Rust ; une modification front écrit et exécute ses tests Vitest, sans relancer une suite
Rust qui ne couvre aucun changement.

---

## 5. Vérifications exécutées

```console
$ uv run --quiet --with pyyaml python -c "import yaml; d=yaml.safe_load(open('.github/workflows/opencode.yml')); ..."
YAML OK; jobs: ['issue', 'comment']
issue   perms= {'id-token': 'write', 'actions': 'write'} timeout= 90
        cache with: {'cache-on-failure': True, 'cache-all-crates': True, 'add-job-id-key': False}
        nextest step present
comment perms= {'id-token': 'write', 'actions': 'write'} timeout= 60
        cache with: {'cache-on-failure': True, 'cache-all-crates': True, 'add-job-id-key': False}
        nextest step present

$ cargo fmt --all -- --check && echo "cargo fmt OK"
cargo fmt OK

$ opencode run --model opencode-go/deepseek-v4.1-flash --format json "Reply with exactly: PONG"
$ opencode session export <sid> | jq '.info.tokens'
# tour 1 : input 12292, cache_read    0 → TOTAL 12292
# tour 2 : input  8196, cache_read 4096 → TOTAL 12292

$ git diff --stat
 .github/workflows/opencode.yml |  19 ++
 .opencode/agent/reviewer.md    |   5 +-
 AGENTS.md                      | 467 +++++++++-----------------------------
 apps/api/AGENTS.md             |  10 +
 apps/backoffice/AGENTS.md      |   5 +
 apps/backoffice/CLAUDE.md      | 147 +-----------
 apps/indexer/AGENTS.md         |   4 +
```

Aucun fichier Rust, TypeScript ou SQL modifié : le build et les tests existants ne sont pas
concernés (le formatage est vérifié ci-dessus). `package-lock.json` était déjà modifié avant
l'intervention et n'est pas inclus dans le commit.

Mesure du prompt (méthode §2, deux tours pour neutraliser l'effet de cache) :

| | avant | après |
|---|---|---|
| input | 12 262 | 8 196 / 12 292 |
| cache read | 3 200 | 4 096 / 0 |
| **total** | **15 462** | **12 292** |

---

## 6. Choix restants (décisions produit — non implémentés)

1. **Effort de raisonnement du modèle** (`variant: max` dans le job `issue`). C'est le plus gros
   levier restant sur la latence : 80 à 148 pas par issue. Choisir explicitement un variant plus
   rapide (ou `max` seulement pour les issues `complex`) est un arbitrage qualité/coût qui vous
   revient.
2. **Skills de design** (`.agents/skills/`, 1,7 Mo, `design-taste-frontend` = 29 Ko de `SKILL.md`,
   plus `redesign-existing-projects`). Leurs descriptions coûtent ~300 tokens à **chaque** pas et
   rien dans ce repo ne les utilise. Les retirer (ou les réserver aux issues UI) est une décision
   d'outillage.
3. **Timeout du job `comment`** : 60 min, alors que des runs ont été annulés à 60 min
   (39443375938) et qu'un run `issue` a duré 56 min. Passer à 90 min supprime les annulations
   stériles, au prix de runs plus longs en cas de blocage.
4. **Périmètre de validation** : `clippy --workspace` + `nextest --workspace` à chaque run, même
   pour une modification de `apps/backoffice`. Restreindre aux paquets touchés (`-p api`,
   `-p indexer`, `-p parsers`) réduirait encore de plusieurs minutes, mais allège la garantie
   « ce que la CI fera ».
5. **Préchauffage planifié du cache** : un job nocturne sur `main` (`cargo clippy` +
   `cargo nextest --no-run`) garderait la clé de restauration chaude même après une PR qui change
   `Cargo.lock` (sinon premier run = clippy à froid).
6. **Sous-agent `reviewer`** : 68 des 148 pas et 25,7 min. Le garder (qualité) ou le fusionner dans
   la session principale est un arbitrage, pas une optimisation technique.
7. **`package.json` racine** : 4 `devDependencies` Tailwind sans `name`, sans script, non
   référencées par les apps (le backoffice a les siennes). Le supprimer retirerait aussi une entrée
   npm de Dependabot — impact sur la politique de dépendances, donc non fait ici.
8. **Hors repo** (configuration globale, non modifiable ici) : le MCP `semble` est lancé via `uvx`
   (démarrage à froid sur un runner neuf), et la CLI opencode des runs CI était en **1.19.3** alors
   que la locale est en **2.0.20** ; aligner/pinner la version est probablement le meilleur rapport
   gain/risque restant.

---

## 7. Risques et points d'attention

- `actions: write` élargit les droits du `GITHUB_TOKEN` du job (gestion des caches/workflows). C'est
  la seule façon connue de sauvegarder un cache après la restriction des permissions ; le jeton
  n'est pas exposé à l'agent par défaut (`persist-credentials: false`), et `ci.yml` reste sur une
  clé distincte.
- `apps/backoffice/CLAUDE.md` porte le marqueur `<!-- rtk-instructions v2 -->` : un `rtk init` local
  peut le régénérer. La version compacte conserve volontairement ce marqueur.
- La suppression de contenu de `AGENTS.md` est un **déplacement** : chaque puce retirée a été
  retrouvée dans un guide module ou un fichier de référence (vérification fichier par fichier).
