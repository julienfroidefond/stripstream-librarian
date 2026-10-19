# Audit — couverture des tests de non-régression

- **Date** : 2026-10-19
- **Référence** : `main` @ `ddd614fb` (inclut le fix #112 via PR #113)
- **Périmètre** : tout le workspace — `crates/*`, `apps/indexer`, `apps/api`, `apps/backoffice`, `apps/docs`
- **Objet** : vérifier que chaque feature est réellement protégée contre les régressions, pas seulement « présente ».

> Le doute initial portait sur les *providers de métadonnées*. C'est traité en détail en
> [§5](#5-focus--la-chaîne-provider-de-métadonnées-112), mais l'audit a été étendu à l'ensemble des
> features, comme demandé.

---

## 1. Méthode

1. Recensement de **tous** les tests : `#[test]`, `#[tokio::test]`, `#[sqlx::test]` (Rust) et `it()`/`test()`
   (Vitest + Playwright).
2. **Mapping fichier source ↔ tests colocalisés** : un fichier à « 0 test inline » n'est *pas* forcément
   non testé (les tests vivent souvent dans un dossier `tests/` voisin, ex. `series/tests/mod.rs`).
   Chaque « 0 » ci-dessous a été recontrôlé par recherche globale du nom de module dans les fichiers de test.
3. Vérification de **ce que la CI exécute réellement** — un test qui ne tourne jamais n'est pas de la couverture.

---

## 2. Volumétrie

| Composant | Tests | dont DB (`#[sqlx::test]`) |
|---|---:|---:|
| `crates/parsers` | 164 | 0 |
| `crates/core` | 32 | 1 |
| `crates/notifications` | 6 | 0 |
| `apps/indexer` | 40 | 29 |
| `apps/api` | 973 | 487 |
| **Total Rust** | **~1 215** | **517** |
| `apps/backoffice` | ~995 (`it`/`test`) + 16 (e2e) | — |
| `apps/docs` | 0 (build seulement) | — |

Répartition saine, très orientée DB côté API (487 tests `#[sqlx::test]`, chacun sur une base migrée jetable).

> **Snapshot *avant* les ajouts** : cette table décrit l'état au moment de l'audit. Les lots A→E ont
> depuis ajouté **103 tests** (56 pour A/B/C en [§9](#9-mise-en-œuvre--lots-a-b-et-c), 47 pour E en
> [§10](#10-mise-en-œuvre--lot-e)) — voir ces sections pour le détail.

---

## 3. Ce qui tourne réellement en CI (et ce qui n'y tourne pas)

| Workflow | Déclencheur | Contenu |
|---|---|---|
| `ci.yml` — *Rust (fmt · clippy · test)* | **PR only**, si `apps/api/**`, `apps/indexer/**`, `crates/**`, `Cargo.*`, `infra/migrations/**`, `rust-toolchain*` | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest run --workspace` **avec Postgres + libpdfium** |
| `ci-web.yml` — *Backoffice (tsc · lint · tests)* | **PR only**, si `apps/backoffice/**` | `tsc --noEmit`, `lint`, `npm run test:unit` |
| `ci-web.yml` — *Docs (build)* | **PR only**, si `apps/docs/**` | `npm run build` |

### Points d'attention sur le dispositif CI

- **La CI ne tourne pas sur les push vers `main`** (volontaire). Conséquence : le check
  `Rust (fmt · clippy · test)` **doit** rester un required status check sur la branche protégée, sinon un
  PR peut être fusionné sans validation. Un push direct / une fusion admin passe le gate.
- **Les tests e2e Playwright ne tournent jamais en CI** (ni dans `ci-web.yml`, ni dans `deploy.yml` — qui
  filtre même les fichiers de test dans sa détection de changements). Ils ne valent donc que
  localement, exécutés à la main. C'est le plus gros trou *structurel* du dispositif.
- **`metadata_providers/tests/providers_e2e.rs`** : 9 tests `#[ignore]` (appels réseau réels). Exclus de
  `nextest` par défaut — normal, mais ils ne protègent rien automatiquement.
- Les tests PDF (`crates/parsers/src/pdf.rs`) **skippent silencieusement** si `libpdfium` est absent ;
  la CI l'installe, donc ils tournent en CI mais pas forcément en local sans setup.

---

## 4. Matrice de couverture par domaine

### 4.1 `crates/parsers` — 164 tests ✅ globalement solide

| Zone | Tests | Verdict |
|---|---|---|
| Extraction de volume (`volume.rs`, via `lib.rs`) | ~80 | ✅ très complet (T/Tome/Vol/#, ranges, accents, faux positifs) |
| Détection de format, `volume_type` (regular/hs/oneshot/integral), sous-dossiers | ~45 | ✅ |
| `matching.rs` (matching release ↔ série) | 20 | ✅ |
| PDF (page count, chiffrement, thumbnail) | 7 | ✅ (mais skip si pdfium absent) |
| Cache d'index CBZ | 2 | ⚠️ correct |
| **`metadata.rs`** (métadonnées d'archive : `analyze_book`, `parse_metadata`, `page_count_only`) | 0 → **7** | ✅ **traité — voir [§10](#10-mise-en-œuvre--lot-e)** |
| **`epub.rs`** (extraction de pages EPUB) | 0 → **14** | ✅ **traité — voir [§10](#10-mise-en-œuvre--lot-e)** |
| **`cbr.rs`** (extraction de pages CBR hors listing) | 0 → **6** | ✅ **traité — voir [§10](#10-mise-en-œuvre--lot-e)** (fallback ZIP→CBZ ; succès RAR non générable) |
| `zip.rs` | 0 inline | ⚠️ couvert indirectement via le cache CBZ |

> **Correction d'audit** : contrairement à ce qu'indiquait la première version, `crates/parsers`
> ne lit **aucun `ComicInfo.xml`** (aucune occurrence de « ComicInfo » dans le crate). La
> métadonnée d'archive est dérivée du **nom de fichier/dossier** (`metadata.rs`). Le gap était donc
> `metadata.rs` au sens « extraction de métadonnées », désormais couvert (§10).

### 4.2 `crates/core` — 32 tests ⚠️ inégal

| Zone | Tests | Verdict |
|---|---|---|
| `paths.rs` (remap/unmap `/libraries` ↔ hôte) | 7 | ✅ chemin critique bien couvert |
| `fingerprint.rs` | 6 | ✅ |
| `config.rs` (env, défauts) | 6 | ✅ |
| `schedule.rs` | 4 | ✅ |
| `disk.rs`, `heartbeat.rs`, `http.rs` | 3 + 3 + 3 | ✅ |
| **`settings.rs`** (`load_setting` / `load_setting_or_default`) | **4** | ✅ **déjà couvert** par `apps/api/src/tests/core_settings.rs` (audit initial périmé) — voir §10 |

### 4.3 `crates/notifications` — 6 tests ⚠️

Formatage des événements, `escape_html`, activation par événement : ✅. L'envoi réseau (`send_telegram`) n'est pas testé — acceptable.

### 4.4 `apps/indexer` — 40 tests ❌ déséquilibré, le pipeline cœur est découvert

| Zone | Tests | Verdict |
|---|---|---|
| `scanner.rs` (phase 1, déduplication, `volume_type`, `get_or_create_series_id`, restauration de progression) | 23 | ✅ la partie *la plus critique* est bien couverte |
| `batch.rs` (flush `UNNEST` : events, page_counts, thumbnails, parse_errors) | 7 | ✅ |
| `scheduler.rs` (jobs DDL) | 4 | ⚠️ partiel |
| `watcher.rs` | 3 | ⚠️ partiel |
| `job.rs` (types de jobs API-only) | 3 | ✅ |
| **`analyzer.rs`** (phase 2 : page_count + génération de thumbnail WebP) | 0 → **11** | ✅ **traité — voir [§9](#9-mise-en-œuvre--lots-a-b-et-c)** |
| **`converter.rs`** (CBR → CBZ) | 0 → **3** | ✅ **traité — voir [§9](#9-mise-en-œuvre--lots-a-b-et-c)** (chemins d'erreur ; succès non générable sans encodeur RAR) |
| **`worker.rs`** (boucle d'orchestration + watcher + scheduler) | **0** | ❌ non testé |
| **`utils.rs`** (remap chemins indexer) | **0** | ⚠️ doublon partiel de `core::paths` (testé) |
| **`scanner/archive.rs`**, **`scanner/discovery.rs`**, **`scanner/mtime.rs`** | **0** | ❌ découverte décomposée, non couverte unitairement |
| `api.rs` (health) | 0 | ⚠️ mineur |

> Le scanner (phase 1) est bien protégé, mais la **phase 2 (analyse/thumbnails) et la conversion CBR→CBZ**,
> qui sont des transformations de fichiers avec effets de bord, n'ont **aucun** test.

### 4.5 `apps/api` — 973 tests (487 DB) ✅ globalement très bon, quelques features orphelines

**Bien couvert :**

| Domaine | Tests | Fichiers |
|---|---:|---|
| Utilisateurs / auth / tokens / scopes / impersonation | 50 | `users/tests/{auth,tokens}.rs` |
| Downloads : Prowlarr (matching + wiremock), qBittorrent, import torrent, détection | 104 | `downloads/tests/*` |
| Metadata : handlers, batch auto-match, gaps, config, sync, refresh (DTO) | 88 | `metadata/tests/*` |
| Providers de métadonnées (7 providers + wiremock) | 103 | `metadata_providers/tests/*` |
| Séries : liste, création, cleanup/prune, ratings, related | 66 | `series/tests/*` |
| Books : renommage, download, pages, liste | 55 + **E3** | `books/tests/*`, `books/pages.rs` (inline) |
| Réglages | 45 | `tests/settings.rs` |
| Libraries | 34 | `tests/libraries.rs` |
| Genres | 32 | `tests/genres.rs` |
| Stats | 28 | `tests/stats.rs` |
| Reading lists | 25 | `reading_lists/tests.rs` |
| Reading : status match/push | 24 | `reading/tests/*` |
| Search | 21 | `tests/search.rs` |
| Authors | 20 | `tests/authors.rs` |
| Discovery | 9 | `integrations/discovery/tests/mod.rs` |
| AI tagging | 14 | `tests/ai_tagging.rs` |
| Diagnostics (handlers + sous-modules) | 11 + **E4** | `tests/diagnostics.rs` |
| Middleware API (auth, rate limit) | 10 | `tests/api_middleware.rs` |
| Jobs (index, lifecycle, poller) | 22 | `jobs/tests/*` |

**Features sans aucun test (vérifié : nom du module absent de tout fichier de test) :**

| Feature | Fichier source | Risque |
|---|---|---|
| **Sync Komga** (`sync_komga_read_books`, rapports) | `integrations/komga.rs` | ✅ **traité (§9)** — 8 tests, **1 bug réel corrigé** |
| **Fusion de séries** (`merge_series`) | `series/merge.rs` | ✅ **traité (§9)** — 6 tests, **1 bug réel corrigé** |
| **Recommandations** | `series/recommendations.rs` | ✅ **traité (§9)** — 8 tests |
| **Rebuild / regenerate thumbnails** (endpoints jobs) | `books/thumbnails.rs` | ✅ **traité (§9)** — 5 tests |
| **Sync AniList** | `integrations/anilist.rs`, `integrations/anilist_sync.rs` | ❌ (seul `anilist_rating_push.rs` a 3 tests) |
| **Intégration Telegram** (hors monitor) | `integrations/telegram.rs` | ❌ |
| **Poll RSS** | `downloads/rss_poll.rs` | ✅ **traité (§9)** — 10 tests |
| **Orchestration refresh métadonnées** | `metadata/refresh.rs` | ⚠️ seuls les DTO sont testés (9), pas le flux |

**Couverture partielle / faible :**

| Zone | Tests | Remarque |
|---|---:|---|
| `books/pages.rs` (rendu + **double cache** mémoire LRU + disque) | 4 | le cache est le point sensible, peu couvert |
| `downloads/import_pipeline.rs`, `downloads/worker.rs` | 0 | le *driver* d'import n'est pas testé (le détail `torrent_import` l'est) |
| `downloads/{blacklist,missing,target,ranged,http_range}.rs` | 0 | utilitaires de download |
| `downloads/ddl_provider/{handlers,types,search,fourtoutici}.rs` | 0 | `service.rs` (14 tests) couvert, le reste non |
| `diagnostics/{database,integrity,jobs,probe,runtime,storage}.rs` | 0 | 11 tests au niveau handler seulement |
| `series/archive.rs` | 0 | partiellement couvert via `series/tests/cleanup.rs` |
| `reading/progress.rs` | 0 | couvert indirectement (`stats`, `search`, `reading_lists`) |
| `series/favorites.rs` | 0 | référencé dans `reading_lists/tests.rs` |

### 4.6 `apps/backoffice` — ~995 tests unitaires ⚠️ excellent en unit, e2e hors CI

- **Unitaires (Vitest)** : ~995 `it` sur 117 fichiers — couverture large (composants, `lib/api*`, i18n,
  formats, hooks). Très bon niveau.
- **e2e (Playwright)** : 16 tests (`smoke`, `flows`, `flows-business`, `interactions`) — **jamais exécutés
  en CI**. Donc les parcours de bout en bout (login, navigation, actions) ne sont pas protégés.

### 4.7 `apps/docs`

Build validé en CI ; pas de tests de contenu (normal). ⚠️ Rien ne détecte une page de doc cassée ou un
lien mort autrement que par le build Astro.

---

## 5. Focus — la chaîne « provider de métadonnées » (#112)

### Ce qui est couvert

| Maillon | Test | Niveau |
|---|---|---|
| `canonical_provider_name` → alias SC = `senscritique` | `canonical_provider_name_normalizes_every_discovery_source` | unit ✅ |
| `resolve_provider` accepte tous les alias / rejette les inconnus | 3 tests dans `metadata_providers/mod.rs` | unit ✅ |
| Toute source de `SUPPORTED_TRENDING_PROVIDERS` → provider implémenté | `every_supported_discovery_source_maps_to_a_provider` | unit ✅ |
| `add_to_library` stocke le provider canonique | `test_add_to_library_discovery_source_normalization` (5 sources) | DB ✅ |
| `filter_already_owned` canonicalise (+ pas de sur-filtrage) | 2 tests | DB ✅ |
| `search_metadata` rejette un provider inconnu en 400 (fin du faux « Google Books ») | `search_metadata_rejects_unknown_provider` | DB ✅ |
| Migration `0129` répare les liens legacy (merge + approved gagne) | 2 tests | DB ✅ |
| Front : la modale n'injecte plus la suggestion | 2 tests `SeriesAddModal` | unit ✅ |

### Les trous (par risque décroissant)

1. **`refresh_link` — zéro test.** C'est *littéralement* la fonction du symptôme #112 :
   `apps/api/src/metadata/refresh_sync.rs:49` produit `Unknown provider: senscritique_bd`. Aucun test ne
   l'appelle. Sa décision est déléguée à `resolve_provider()` (testé), mais **le câblage ne l'est pas** :
   rien ne casse si demain quelqu'un repasse `refresh_link` sur `get_provider()` direct, ou casse le message.
   Testable sans réseau (la résolution se fait avant tout appel réseau). → ✅ **traité (A1)**
2. **`available_providers()` ↔ `get_provider()` : parité non testée.** `resolve_provider_accepts_real_providers`
   re-hardcode les 7 noms au lieu de s'appuyer sur `available_providers()`. Donc si on ajoute un provider dans
   la liste sans l'implémenter (ou l'inverse), il s'affiche dans le BO puis 400 à l'usage. La parité n'est
   vérifiée que pour les sources de découverte, pas pour la liste exposée. → ✅ **traité (A2)**
3. **`sync_books_from_links` : canonicalisation + fallback `google_books` volontaire non testés**
   (`apps/api/src/metadata/sync.rs:253-256`). Le fallback est intentionnel (liens `prowlarr`), mais non verrouillé.
   → ✅ **traité (A3)**
4. **Incohérence fonctionnelle : `batch_sync::search_and_evaluate` (`apps/api/src/metadata/batch_sync.rs:26`)
   utilise `get_provider()` direct, sans canonicalisation.** Contrairement à search/refresh. Un alias legacy
   en base y donnerait `Unknown provider: …`. L'`AGENTS.md` parle de `resolve_provider` comme « single
   chokepoint » — ce n'est vrai que pour search/refresh. → ✅ **traité (A4)** : `batch_sync` canonicalise
   désormais via `resolve_provider`, et `AGENTS.md` a été corrigé (A5).
5. Mineur : `search_metadata` avec un alias de découverte (résolution → `senscritique`) n'est pas testé au
   niveau handler, et la canonicalisation du cache `trending` (`integrations/discovery/mod.rs:204+`) non plus.

### Verdict sur ce doute

Il est fondé, mais pas au sens « on ne teste rien » : la canonicalisation / résolution / migration sont bien
couvertes. Le trou réel, c'est que **les deux fonctions qui *utilisent* la résolution (`refresh_link`,
`sync_books_from_links`) ne sont pas testées** — et c'est précisément là que le bug s'était manifesté. Comme
les providers tapent des API externes, on ne peut pas les tester bout-en-bout, mais on peut (et devrait)
tester la décision + le câblage sans réseau.

---

## 6. Gaps priorisés (transverse)

| Priorité | Zone | Ce qui n'est pas couvert | Risque |
|---|---|---|---|
| **P1** | `apps/indexer` | `analyzer.rs` (phase 2 : page_count + thumbnails), `converter.rs` (CBR→CBZ) | Transformation de fichiers, cœur du produit, 0 test |
| **P1** | `apps/api` | `series/merge.rs` (fusion, destructive), `integrations/komga.rs` (sync) | Perte de données / feature complète non protégée |
| **P1** | `apps/api` | `refresh_link` + parité `available_providers()` (cf. §5) | Régression #112 non verrouillée |
| **P1** | CI | e2e Playwright jamais exécutés en CI | Aucun parcours utilisateur protégé |
| **P2** | `apps/indexer` | `worker.rs`, `scanner/{archive,discovery,mtime}.rs` | Orchestration + découverte non couvertes |
| **P2** | `apps/api` | `books/thumbnails.rs`, `series/recommendations.rs`, `integrations/anilist*.rs`, `integrations/telegram.rs`, `downloads/rss_poll.rs` | Features exposées, 0 test |
| **P2** | `apps/api` | `metadata/refresh.rs` (orchestration), `books/pages.rs` (double cache) | ✅ `books/pages.rs` **traité (§10)** ; `metadata/refresh.rs` encore ouvert |
| **P3** | `crates/parsers` | `epub.rs`, `metadata.rs`, `cbr.rs` (extraction) | ✅ **traité (§10)** |
| **P3** | `crates/core` | `settings.rs` | ✅ **déjà couvert** (`tests/core_settings.rs`) — audit périmé |
| **P3** | `apps/api` | `diagnostics/*`, `downloads/ddl_provider/*`, `downloads/{worker,import_pipeline}`, utilitaires download | ✅ `diagnostics/*` **traité (§10)** ; le reste ouvert |
| — | — | `providers_e2e` `#[ignore]`, envois réseau Telegram | Assumé (réseau) |

---

## 7. Plan d'action proposé

### Lot A — Providers de métadonnées (rapide, sans réseau) — *le doute initial* — ✅ **implémenté (§9)**

- **A1.** `refresh_link` rejette un provider inconnu avec `Unknown provider: <nom>` et **ne retombe pas**
  sur Google Books (test le moins cher, direct sur le bug #112).
- **A2.** Test de parité `available_providers()` ↔ `resolve_provider()`/`get_provider()` (anti-dérive).
- **A3.** Seam testable pour `sync_books_from_links` : extraire `resolve_link_provider()`
  (canonique + fallback) et tester alias→canonique et `prowlarr`→`google_books`.
- **A4.** Réutiliser la canonicalisation dans `batch_sync` pour supprimer l'incohérence.
- **A5.** (doc) Corriger `AGENTS.md` : `resolve_provider` n'est *pas* le chokepoint unique aujourd'hui.

### Lot B — Indexer (phase 2 & conversion) — ✅ **implémenté (§9)**

- **B1.** `analyzer.rs` : tests sur page_count + génération/écriture de thumbnail (fixtures CBZ/PDF), y
  compris le cas d'échec de parsing (`parse_errors`).
- **B2.** `converter.rs` : conversion CBR→CBZ (fiche de sortie, idempotence, entrée invalide).

### Lot C — API, features orphelines — ✅ **implémenté (§9)**

- **C1.** `series/merge.rs` : fusion (mapping des liens, dédoublonnage, conservation approved/AniList).
- **C2.** `integrations/komga.rs` : sync read (wiremock) + génération de rapport.
- **C3.** `books/thumbnails.rs` : création des jobs rebuild/regenerate.
- **C4.** `series/recommendations.rs`, `downloads/rss_poll.rs` (wiremock).

### Lot D — CI & dispositif

- **D1.** Exécuter les e2e Playwright en CI (au moins un smoke ciblé) ou les intégrer à `ci-web.yml`.
- **D2.** Vérifier / documenter que `Rust (fmt · clippy · test)` et `Backoffice (tsc · lint · tests)` sont
  bien des required status checks (la CI ne tourne pas sur `main`).

### Lot E — Correctifs de couverture résiduels (P3) — ✅ **implémenté (§10)**

- E1. `epub.rs` / `metadata.rs` / `cbr.rs` — fixtures générées. ✅
- E2. `core/settings.rs` — **déjà couvert** par `apps/api/src/tests/core_settings.rs` (audit périmé, aucun test ajouté). ✅
- E3. `books/pages.rs` — helpers de rendu + invalidation du double cache (bout en bout). ✅
- E4. `diagnostics/*` — dégradation gracieuse + agrégats par sous-module. ✅

---

## 8. Annexe — commandes utiles

```bash
# Toute la suite Rust (unit + DB) — nécessite Postgres local sur 6432
DATABASE_URL="postgres://stripstream:stripstream@localhost:6432/stripstream" cargo test --workspace

# Idem via nextest (comme la CI)
cargo nextest run --workspace

# Un module / un test
cargo test -p api -- series::tests
cargo test test_name

# Tests providers en direct (réseau, normalement ignorés)
cargo test -p api -- --ignored providers_e2e

# Backoffice
cd apps/backoffice && npm run test:unit      # Vitest
cd apps/backoffice && npx playwright test    # e2e (jamais en CI aujourd'hui)

# Docs
cd apps/docs && npm run build
```

---

## 9. Mise en œuvre — lots A, B et C

**Statut** : lots A, B et C **implémentés et verts** (56 tests ajoutés). Le **lot E** l'est aussi — voir
[§10](#10-mise-en-œuvre--lot-e). Seul le lot D (CI) reste ouvert.

Convention respectée : un test par comportement, colocalisé (`#[path = "tests/<x>.rs"] mod tests;` en fin de
fichier), fixtures générées à la volée (aucune dépendance à `libraries/`, gitignoré). Les tests réseau
passent par `wiremock` (déjà en dev-dependency de `apps/api`) — aucun appel sortant réel.

### Lot A — chaîne « provider de métadonnées » (5 tests)

| # | Test | Fichier |
|---|---|---|
| A1 | `refresh_link_rejects_unknown_provider_without_fallback` | `metadata/tests/refresh_sync.rs` |
| A1 | `refresh_link_accepts_discovery_source_alias` | `metadata/tests/refresh_sync.rs` |
| A2 | `available_providers_are_all_implemented_and_resolvable` | `metadata_providers/mod.rs` |
| A3 | `resolve_link_provider_name_canonicalizes_discovery_sources` | `metadata/tests/sync.rs` |
| A3 | `resolve_link_provider_name_falls_back_to_google_books` | `metadata/tests/sync.rs` |

- **A3** a introduit le seam `metadata::sync::resolve_link_provider_name()` (canonicalisation + fallback
  legacy `google_books`), rendant la décision de `sync_books_from_links` testable sans réseau.
- **A4** : `batch_sync::search_and_evaluate` canonicalise désormais via `resolve_provider` puis
  `provider.name()` — plus d'`Unknown provider: <alias>` en base.
- **A5** : `AGENTS.md` (section *Metadata Providers*) corrigé : `resolve_provider` est le chokepoint de
  search/batch/refresh ; le chemin legacy `sync::resolve_link_provider_name` conserve le fallback.

### Lot B — indexer (14 tests)

| Fichier | Tests | Contenu |
|---|---:|---|
| `apps/indexer/src/analyzer.rs` | 11 | 7 unitaires (`detect_image_ext`, `book_format_from_str`, `generate_thumbnail` WebP/pas d'upscale/format explicite, `fast_jpeg_decode`, `save_raw_image`+`resize_raw_to_thumbnail`) + 4 `#[sqlx::test]` (`analyze_library_books` page_count+thumbnail, `parse_errors` sur archive corrompue, `regenerate_thumbnails` nettoie les fichiers périmés, `cleanup_orphaned_thumbnails`) |
| `apps/indexer/src/converter.rs` | 3 | `convert_book` : pas d'enregistrement `book_files`, format non-CBR, échec `unrar` rapporté **sans toucher la DB** |

- Dépendances de test ajoutées à la racine (`Cargo.toml` → `[workspace.dependencies]`) : `tempfile = "3"`
  et `zip = "8"` (features `deflate`), consommées en `[dev-dependencies]` de l'indexer — conformément à la
  règle « deps externes déclarées à la racine ».
- **Limite assumée** : le chemin *succès* CBR→CBZ n'est pas testable (aucun encodeur RAR disponible — ni
  binaire `rar`/`unrar`/`7z`, ni encodeur dans un crate) ; les tests couvrent les gardes et les chemins
  d'erreur de `convert_book`, qui sont le risque réel (fichier DB créé à tort, format accepté à tort).

### Lot C — features API orphelines (37 tests)

| # | Fichier source | Tests | Fichier de test |
|---|---|---:|---|
| C1 | `series/merge.rs` | 6 | `series/tests/merge.rs` |
| C2 | `integrations/komga.rs` | 8 | `integrations/tests/komga.rs` |
| C3 | `books/thumbnails.rs` | 5 | `books/tests/thumbnails.rs` |
| C4 | `series/recommendations.rs` | 8 | `series/tests/recommendations.rs` |
| C4 | `downloads/rss_poll.rs` | 10 | `downloads/tests/rss_poll.rs` |

- **C1** pilote le *vrai* handler `merge_series` : auto-fusion interdite, fusion inter-bibliothèques
  interdite, source inconnue → 404, déplacement complet + suppression de la source, conflit de provider
  (métadonnées), conflit de `available_downloads`.
- **C2** : Komga faked via `wiremock` ; vérifie le matching (série exacte, repli sur le titre seul),
  l'idempotence (`already_read`), les non-matches, la pagination multi-pages, l'erreur HTTP, et
  l'historique des rapports.
- **C3** : création effective des jobs `thumbnail_rebuild` / `thumbnail_regenerate` (périmètre bibliothèque
  vs global, absence de bibliothèque → erreur).
- **C4 recommendations** : sans utilisateur → vide ; pondération auteur > genre commun ; exclusion des
  séries déjà commencées ; respect des restrictions de genre ; bonus liste de lecture partagée ; bonus
  score communautaire (liens approuvés) ; plafond `limit` ; sources = N plus récentes.
- **C4 rss_poll** : gardes du handler (Prowlarr non configuré, `library_id` invalide/inconnue, dédup d'un
  job en cours global et par bibliothèque), `fetch_rss_releases` (dédup par GUID multi-indexeurs,
  indexeur en échec isolé), et le pipeline de bout en bout (`process_rss_poll` : aucun target, détection +
  stockage `available_downloads` + événement `downloads_found`, cas « rien trouvé » + `downloads_not_found`).

### Deux bugs réels découverts et corrigés

1. **`series/merge.rs` — fusion de deux séries ayant chacune un `available_downloads`.**
   `UPDATE available_downloads SET series_id = $target WHERE series_id = $source AND id NOT IN
   (SELECT id FROM available_downloads WHERE series_id = $target)` : le `NOT IN` était toujours vrai (il
   comparait la ligne à elle-même), donc les deux lignes migraient et violaient `UNIQUE(series_id)` → 409.
   Corrigé par une condition d'existence : `AND NOT EXISTS (SELECT 1 FROM available_downloads WHERE
   series_id = $target)`. Verrouillé par `merge_series_keeps_target_download_when_both_have_one`.

2. **`integrations/komga.rs` — endpoint de sync entièrement cassé.**
   La requête de lookup local lisait `COALESCE(series, '') FROM books`, or la colonne `books.series` a été
   **supprimée par la migration 0071** (`series_id` FK depuis 0070). Tout appel à `sync_komga_read_books`
   échouait donc en 500. Corrigé par une jointure explicite :
   `FROM books b LEFT JOIN series s ON s.id = b.series_id`. Le test de bout en bout
   `sync_matches_local_books_and_marks_them_read` couvre désormais ce chemin.

> Ces deux cas illustrent la thèse de l'audit : le trou de couverture n'était pas cosmétique — deux
> features exposées étaient réellement cassées sans qu'aucun test ne s'en aperçoive.

### Reste à faire

- **Lot D** : e2e Playwright en CI (`ci-web.yml`), et required status checks vérifiés/documentés.

---

## 10. Mise en œuvre — lot E

**Statut** : lot E **implémenté et vert** (47 tests ajoutés, plus 2 écarts d'audit corrigés). Total cumulé
des lots A→E : **103 tests**.

Validation finale (après lots A→E) : `cargo fmt -- --check` ✅, `cargo clippy --workspace --all-targets
-- -D warnings` ✅, `cargo test --workspace --no-fail-fast` ✅ **1309 passed / 0 failed / 9 ignored**
(contre 1262 avant le lot E).

| # | Cible | Tests | Fichier de test |
|---|---|---:|---|
| E1 | `crates/parsers/src/epub.rs` | 14 | module `tests` inline |
| E1 | `crates/parsers/src/metadata.rs` | 7 | module `tests` inline |
| E1 | `crates/parsers/src/cbr.rs` | 6 | module `tests` inline |
| E3 | `apps/api/src/books/pages.rs` | 11 | module `tests` inline (étendu) |
| E4 | `apps/api/src/tests/diagnostics.rs` | 9 | étendu |
| E2 | `crates/core/src/settings.rs` | 0 | **déjà couvert** (`apps/api/src/tests/core_settings.rs`) |

### E1 — `crates/parsers` (27 tests)

Fixtures générées à la volée (aucune dépendance à `libraries/`, gitignoré) : EPUB construits avec le crate
`zip`, JPEG minimal `[0xFF, 0xD8, 0xFF, 0xD9]`.

- **`epub.rs` (14)** : résolution de chemin (`resolve_epub_path`, `normalize_epub_path`, `percent_decode_epub`),
  décodage d'entités XML (`decode_xml_entities`, `epub_hex_val`), parsing OPF (`parse_epub_opf` : manifest/spine,
  métadonnées, ordre de lecture), index d'images (`build_epub_image_index`, `get_epub_image_index`, invalidation),
  analyse et extraction de page (`analyze_epub`, `extract_epub_page`, `parse_epub_page_count`).
- **`metadata.rs` (7)** : `parse_metadata_fast` / `parse_metadata` (volume, titre nettoyé, `volume_type`),
  `page_count_only` (PDF/CBZ/EPUB), `analyze_book` (dispatch par format, archive absente → erreur,
  archive corrompue → pas de panique).
- **`cbr.rs` (6)** : `is_not_rar_error`, `convert_cbr_to_cbz` (non-RAR → fallback ZIP, entrée absente → erreur,
  listing CBR absent). **Limite assumée** : pas d'encodeur RAR → le succès d'extraction CBR pur n'est pas
  générable ; les chemins d'erreur et le fallback ZIP sont verrouillés.

> **Contrat réel pinné** : un `.cbz` **corrompu** renvoie `page_count = Some(0)` (le fallback streaming
> `raw_zip_list_entries` renvoie `Ok(0)`), `None` uniquement si l'archive est absente/illisible. Le test
> enregistre ce comportement réel plutôt que de le changer.

### E3 — `apps/api/src/books/pages.rs` (11 tests)

- **Unitaires** : `OutputFormat::parse` (alias + rejet), `content_type`/`extension`, `get_cache_path`
  (sharding 2 premiers caractères + extension), `detect_content_type` (PNG/JPEG/WebP magic + octet-stream),
  `parse_filter` (défaut Triangle), `format_matches` (familles d'images), `transcode_image`
  (passthrough sans travail / resize aspect-preserving / forçage JPEG / conteneur WebP `RIFF…WEBP`),
  `file_mtime_ns` (0 si absent), `image_response` (ETag + 304 sur `If-None-Match`).
- **Bout en bout `#[sqlx::test]`** `get_page_serves_memory_cache_then_invalidates_on_file_change` :
  CBZ temporaire + `cache_directory` redirigé vers un `tempdir` ; 1ᵉʳ appel = miss (rendu rouge,
  `page_cache_misses == 1`), 2ᵉ appel = hit mémoire (`page_cache_hits == 1`, mêmes octets + même ETag) ;
  réécriture du CBZ avec une autre image + `set_modified` futur → **double invalidation** (nouvelle clé
  SHA256 → re-rendu bleu, nouvel ETag) et **2 entrées** sur le disque. Verrouille la propriété critique :
  un fichier modifié ne peut jamais servir une page périmée.

### E4 — `apps/api/src/tests/diagnostics.rs` (9 tests)

- `database_diagnostics` : pool injoignable (port 1, `acquire_timeout` 200 ms) → `reachable = false`
  et compteurs à zéro, **jamais 500**.
- `services_diagnostics` : base injoignable → liste vide malgré des heartbeats présents.
- `jobs_diagnostics` : repli sur `index_job_errors` quand `error_opt` est vide ; tri `by_type` du plus actif
  au moins actif + `last_finished_at`.
- `integrity_diagnostics` : les volumes `hs` et les volumes NULL ne sont **pas** comptés comme doublons
  (garde-fou `volume_type = 'regular'`).
- `storage` : `thumbnail_diagnostics` compte les fichiers/size du répertoire configuré ;
  `disk_diagnostics` marque un montage absent `available = false` (dégradation gracieuse).
- `libraries_diagnostics` : mapping des horodatages de planification (scan / refresh metadata / détection /
  push de statut).
- `runtime_diagnostics` : `renders_in_flight` reflète un permit détenu ; RSS renseigné.

> **Correction d'audit** : `crates/core/src/settings.rs` était **déjà couvert** (4 tests dans
> `apps/api/src/tests/core_settings.rs`, déplacés là pour éviter la course sur `DATABASE_URL`) — aucune
> action E2. Et la mention « ComicInfo » était **inexacte** : `crates/parsers` ne lit aucun `ComicInfo.xml`,
> la métadonnée d'archive est dérivée du nom de fichier/dossier.

### Reste ouvert (hors lots A–E)

- **Lot D** (CI/e2e) — cf. §7.
- P2 API : `integrations/anilist*.rs`, `integrations/telegram.rs`, orchestration `metadata/refresh.rs`.
- P2 indexer : `worker.rs`, `scanner/{archive,discovery,mtime}.rs`.
- P3 : `downloads/ddl_provider/*`, `downloads/{worker,import_pipeline}`.
