# apps/backoffice — Interface d'administration (Next.js)

App Next.js 16 avec React 19, Tailwind CSS v4, TypeScript. Port de dev : **7082** (`npm run dev`).

## Structure

```
app/
├── layout.tsx          # Layout global (nav sticky glassmorphism, ThemeProvider)
├── page.tsx            # Dashboard
├── books/              # Liste et détail des livres
├── libraries/          # Gestion bibliothèques
├── jobs/               # Monitoring jobs
├── diagnostics/        # Diagnostic / santé (API, base, caches, miniatures, jobs)
├── tokens/             # Tokens API
├── settings/           # Paramètres
├── components/         # Composants métier
│   ├── ui/             # Composants génériques (Button, Card, Badge, Icon, Input, ProgressBar, StatBox...)
│   ├── BookCard.tsx
│   ├── JobProgress.tsx
│   ├── JobsList.tsx
│   ├── LibraryForm.tsx
│   ├── FolderBrowser.tsx / FolderPicker.tsx
│   └── ...
└── globals.css         # Variables CSS, Tailwind base
lib/
├── api/                # Client API : types DTO + fetch vers l'API Rust, découpé par domaine
│   ├── index.ts        # Barrel (`@/lib/api`)
│   ├── client.ts       # `apiFetch` / `config`
│   └── ...             # libraries, jobs, books, series, metadata, downloads, ...
└── api-handler.ts      # `withRoute` pour les route handlers
```

## Client API (lib/api/)

Tous les appels vers l'API Rust passent par `lib/api` (barrel `lib/api/index.ts`). Le module est
découpé par domaine (`client`, `libraries`, `jobs`, `books`, `series`, `metadata`, `downloads`…) ;
les types DTO vivent dans le fichier du domaine concerné :
- `LibraryDto`, `IndexJobDto`, `BookDto`, `TokenDto`, `FolderItem`

Ajouter les nouveaux endpoints et types dans le module du domaine concerné ; le barrel les
réexporte automatiquement.

`lib/api-handler.ts` expose `withRoute(handler, options)`, enveloppe standard des route handlers
`app/api/**` (extraction du message d'erreur, statut 500 par défaut, options `fallback` / `status` /
`statusFromError` / `onError`). L'utiliser pour tout nouveau handler.

## Composants UI

Les composants génériques sont dans `app/components/ui/`. Utiliser ces composants plutôt que des éléments HTML bruts :

```tsx
import { Button, Card, Badge, Icon, Input, ProgressBar, StatBox } from "@/app/components/ui";
```

## Conventions

- **App Router** : toutes les pages sont des Server Components par défaut. Utiliser `"use client"` seulement pour l'interactivité.
- **Tailwind v4** : config dans `postcss.config.js`, tokens et variables CSS dans `globals.css` (`@theme`). Pas de `tailwind.config.js` (v4 ne le charge pas sans directive `@config`).
- **Thème** : `ThemeProvider` + `ThemeToggle` pour dark/light mode via `next-themes`.
- **Icônes** : composant `<Icon name="..." size="sm|md|lg" />` dans `ui/Icon.tsx` — pas de librairie externe.
- **Navigation** : routes typées dans `layout.tsx` (`"/" | "/books" | "/libraries" | "/jobs" | "/tokens" | "/settings" | "/diagnostics"`).

## Commandes

```bash
npm install
npm run dev    # http://localhost:7082
npm run build
npm run start  # Production sur http://localhost:7082
npm run lint   # ESLint (eslint.config.mjs, flat config next/core-web-vitals)
npx tsc --noEmit
```

Gestionnaire de paquets : **npm** uniquement (`package-lock.json` est suivi et utilisé par la
build Docker `npm ci`). Ne pas introduire `pnpm`/`yarn` (les lockfiles alternatifs sont ignorés
par git mais désynchronisent la CI).

## Tests unitaires (Vitest)

Tests unitaires et de composants dans `tests/unit/` (logique pure, hooks, composants UI extraits). Environnement `jsdom`, alias `@/` résolu via `vitest.config.mts`. Aucune stack requise.

```bash
npm run test:unit        # exécution unique
npm run test:unit:watch  # mode watch
npm run test:coverage    # rapport de couverture (text + html dans coverage/)
```

- `tests/setup.ts` mocke `next/link`, `next/image`, `next/navigation` et `@/lib/i18n/context` (`t` renvoie la clé) ; `@testing-library/jest-dom` est chargé globalement.
- Couvre :
  - logique/`lib` : `format`, `jobStatus`, `ratings`, `searchParams`, `session`, `volumeRanges`, `useEventSource`, `usePopin`, `useMarkRead`, `i18n` (dictionnaires sync + chargement async, parité fr/en, interpolation, `LocaleProvider`/`useTranslation`), `api` (`apiFetch`, `config`, `getBookCoverUrl` et tous les endpoints : librairies, jobs, tokens, livres, séries, métadonnées, reading status, downloads, komga, reading lists, archive, reading overview, telegram) ;
  - composants : `TagInput`, `Modal`, `DeleteConfirmButton`, `MarkReadButton`, `MarkBookReadButton`, `MarkSeriesReadButton`, `TestConnectionButton`, `SeriesResultRow`, `StatCard`, `SeriesGrid`, `MetadataReportCards`, `ReadingStatusReportCards`, `RatingStars`, `JobProgress`, `Pagination`, `ProgressBar`, `ActionsMenu`, `BookCard`, `LibraryForm`, `EditBookForm`, `EditSeriesForm`, `JobRow`, `JobsList`, `ProwlarrCard`, `QBittorrentCard`, `TelegramCard`, `ThemeToggle`/`ThemeSelector`, plus les primitives `ui/` (`Badge`/`StatusBadge`/`JobTypeBadge`/`ProgressBadge`, `Card`/`GlassCard`/`SimpleCard`, `Tooltip`, `CoverFan`, `Button`/`IconButton`, `Input`/`Select`/`Textarea`/`SearchInput`, `Form`, `StatBox`, `Switch`, `ReportStatGrid`, `SettingsCard`/`SettingsField`, `Toast`, `Icon`) ;
  - settings : `AiTaggingCard`, `MetadataProvidersCard`, `StatusMappingsCard`, `RenameFormatCard`, `ArchivesTab`, `TokensTab` (server component + server actions), `AnilistTab`, `KomgaSyncCard`, `TelegramMonitorCard` ;
  - routes/app : `app/health`, `app/login` (formulaire), `app/theme-provider`, et l'ensemble des route handlers `app/api/**` (CRUD, SSE `streams.test.ts`, cas d'erreur data-driven `errors.test.ts`).
- Les tests de route handlers utilisent `// @vitest-environment node` et `tests/unit/app/api/helpers.ts` (`createApiMock()`, `jsonRequest()`, `routeCtx()` retournant `{ params: Promise<any> }`).
- Pour les composants à `fetch` global, stubber `vi.stubGlobal("fetch", fetchMock)` et router par `url`/`method` ; préférer `fireEvent.change`/`fireEvent.blur` pour les champs `{var}` ou number contrôlés.
- `lib/api/**` est couvert par `tests/unit/lib/api-*.test.ts` ; `lib/api-handler.ts` par
  `tests/unit/lib/api-handler.test.ts`.
- Le test de `lib/i18n/context` appelle `vi.unmock("@/lib/i18n/context")` pour rétablir le vrai provider (le setup le mocke globalement).
- Les tests de `lib/session.ts` utilisent `// @vitest-environment node` (`jose` compare les `Uint8Array` par realm, incompatible avec jsdom).
- Importer explicitement `describe`/`it`/`expect`/`vi` depuis `vitest` (pas de globals).
- La CI GitHub Actions (`.github/workflows/ci-web.yml` sur PR, `deploy.yml` avant le build
  d'image sur `main`) lance `npm ci`, `tsc --noEmit`, `npm run lint` et `npm run test:unit` sur
  tout changement `apps/backoffice/`.

## Tests E2E (Playwright)

Suite Playwright organisée par **domaines fonctionnels** dans `tests/e2e/` (la matrice
fonctionnalité → spec → lacunes est dans `tests/e2e/README.md`). Elle nécessite l'API (7080),
l'indexer et PostgreSQL (stack Docker), et lit `ADMIN_USERNAME`/`ADMIN_PASSWORD` ainsi que
`API_BOOTSTRAP_TOKEN` dans `.env.local`.

```bash
npm run test:e2e         # headless, réutilise le dev server sur 7082 s'il tourne
npm run test:e2e:headed  # avec navigateur visible
npm run test:e2e:ui      # mode UI Playwright
npx playwright test catalog/series.spec.ts -g "recherche"   # cibler un test
```

- Dossiers : `shell/`, `dashboard/`, `catalog/`, `metadata/`, `reading/`, `libraries/`,
  `acquisition/`, `operations/`, `admin/`.
- Parcours d'écriture : `catalog/books-write.spec.ts` (édition, téléchargement, suppression,
  verrouillage d'un champ), `catalog/series-write.spec.ts` (édition/verrouillage, renommage,
  suppression via Actions), `catalog/authors-genres.spec.ts` (affectation en masse d'un genre,
  filtre « sans genre » et suppression),
  `catalog/series-merge.spec.ts` (fusion et déplacement du lien de métadonnées),
  `catalog/ratings.spec.ts` (note puis retrait), `libraries/settings-write.spec.ts` (réglages
  restaurés via l'API), `metadata/search-approve.spec.ts` (recherche mockée, approbation,
  déliaison), `metadata/sync-write.spec.ts` (promotion d'un provider et synchronisation depuis
  les métadonnées stockées), `operations/jobs-write.spec.ts` (rebuild ciblé + replay),
  `operations/diagnostics.spec.ts` (purge de cache), `admin/connection-test.spec.ts` (intégrations
  mockées), `admin/tokens.spec.ts` (réassignation de token + renommage inline),
  `admin/user-restrictions.spec.ts` (bascule d'utilisateur actif + genre bloqué),
  `admin/archives-orphans.spec.ts` (orphelin → archive, suppressions), `reading/lists.spec.ts`
  (renommage/réordonnancement/groupement), `reading/progress-write.spec.ts` (lu/non lu, série
  entière avec statut par tome, état d'origine restauré via l'API).
- `auth.setup.ts` (projet `setup`) se connecte via `/api/auth/login`, crée/réutilise le lecteur
  `e2e-reader`, purge les ressources `e2e-*` d'une exécution interrompue, recrée les bibliothèques
  temporaires `e2e-metadata`, `e2e-books` et `e2e-jobs`, enregistre la session dans
  `tests/e2e/.auth/state.json` (ignoré par git) et ajoute le cookie `as_user_id`.
- `global-teardown.ts` supprime les bibliothèques `e2e-*` et purge leurs archives en fin de run.
- `support/api.ts` appelle directement l'API Rust avec `API_BOOTSTRAP_TOKEN` (feature flags,
  nettoyage listes/tokens/users, CRUD bibliothèques/séries, liens et métadonnées de série,
  jobs) ; `support/library.ts` seed une mini-bibliothèque de fixtures (`E2E_SEED_LIBRARY=1`) et
  attend l'indexation ;
  `support/metadata.ts` crée des séries éphémères avec auteurs/genres uniques dans
  `e2e-metadata` et fournit les racines dédiées `e2e-books` / `e2e-jobs` ; `support/books.ts`
  crée un livre ou une série multi-tomes scannée dans `e2e-books` ; `helpers.ts` expose
  `openFirstSeries`, `openSeriesDetail`, `uniqueName`, `escapeRegExp`.
- Les tests sont autonomes et compatibles `fullyParallel` : noms uniques et nettoyage `finally`.
  Ne pas modifier les réglages globaux (langue, feature flags) depuis un test.
- `test.skip(...)` uniquement pour une fonctionnalité désactivée (`downloads_enabled`,
  AllDebrid non configuré) ou une donnée absente (livres, jobs, bibliothèques) — jamais pour
  masquer une régression.
- L'UI est en anglais par défaut ; seuls les libellés de `/login` sont en français.
- La suite n'est **pas exécutée en CI** : elle se lance en local, sur la stack de dev, quand on
  en a besoin. Le rapport HTML est écrit dans `tests/e2e/.report/` (ignoré par git).

## Gotchas

- **Port 7082** : pas le port Next.js par défaut (3000). Défini dans `package.json` scripts (`-p 7082`).
- **API_BASE_URL** : en prod, configuré via env. En dev local, l'API doit tourner sur `http://localhost:7080`.
- **React 19 + Next.js 16** : utiliser les nouvelles APIs (actions serveur, `use()` hook) si disponibles.
- **Pas de gestion d'état global** : fetch direct depuis les Server Components ou `useState`/`useEffect` dans les Client Components.
- **Cache & images** : `staleTimes.dynamic = 30` ; les fetch API utilisent
  `{ next: { revalidate: N } }` (15 s pour les listes, 30-60 s pour les données stables). Les
  images utilisent `unoptimized: true` — servir des WebP pré-optimisés depuis `public/`.
- **Next.js 16 en production (standalone)** : `router.replace()` ne fonctionne pas. Utiliser
  `window.history.replaceState()` + `router.refresh()` (voir `LiveSearchForm.tsx`).

<!-- BEGIN:nextjs-agent-rules -->

## This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->
