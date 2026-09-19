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
└── api.ts              # Client API : types DTO + fonctions fetch vers l'API Rust
```

## Client API (lib/api.ts)

Tous les appels vers l'API Rust passent par `lib/api.ts`. Les types DTO sont définis là :
- `LibraryDto`, `IndexJobDto`, `BookDto`, `TokenDto`, `FolderItem`

Ajouter les nouveaux endpoints et types dans ce fichier.

## Composants UI

Les composants génériques sont dans `app/components/ui/`. Utiliser ces composants plutôt que des éléments HTML bruts :

```tsx
import { Button, Card, Badge, Icon, Input, ProgressBar, StatBox } from "@/app/components/ui";
```

## Conventions

- **App Router** : toutes les pages sont des Server Components par défaut. Utiliser `"use client"` seulement pour l'interactivité.
- **Tailwind v4** : config dans `postcss.config.js` + `tailwind.config.js`. Variables CSS dans `globals.css`.
- **Thème** : `ThemeProvider` + `ThemeToggle` pour dark/light mode via `next-themes`.
- **Icônes** : composant `<Icon name="..." size="sm|md|lg" />` dans `ui/Icon.tsx` — pas de librairie externe.
- **Navigation** : routes typées dans `layout.tsx` (`"/" | "/books" | "/libraries" | "/jobs" | "/tokens" | "/settings"`).

## Commandes

```bash
npm install
npm run dev    # http://localhost:7082
npm run build
npm run start  # Production sur http://localhost:7082
npx tsc --noEmit
```

## Tests unitaires (Vitest)

Tests unitaires et de composants dans `tests/unit/` (logique pure, hooks, composants UI extraits). Environnement `jsdom`, alias `@/` résolu via `vitest.config.mts`. Aucune stack requise.

```bash
npm run test:unit        # exécution unique
npm run test:unit:watch  # mode watch
npm run test:coverage    # rapport de couverture (text + html dans coverage/)
```

- `tests/setup.ts` mocke `next/link`, `next/image`, `next/navigation` et `@/lib/i18n/context` (`t` renvoie la clé) ; `@testing-library/jest-dom` est chargé globalement.
- Couvre :
  - logique/`lib` : `format`, `jobStatus`, `ratings`, `searchParams`, `session`, `volumeRanges`, `useEventSource`, `usePopin`, `i18n` (dictionnaires sync + chargement async, parité fr/en, interpolation, `LocaleProvider`/`useTranslation`), `api` (`apiFetch`, `config`, `getBookCoverUrl` et tous les endpoints : librairies, jobs, tokens, livres, séries, métadonnées, reading status, downloads, komga, reading lists, archive, reading overview, telegram) ;
  - composants : `TagInput`, `Modal`, `DeleteConfirmButton`, `MarkReadButton`, `MarkBookReadButton`, `MarkSeriesReadButton`, `TestConnectionButton`, `SeriesResultRow`, `StatCard`, `SeriesGrid`, `MetadataReportCards`, `ReadingStatusReportCards`, `RatingStars`, `JobProgress`, `Pagination`, `ProgressBar`, `ActionsMenu`, plus les primitives `ui/` (`Badge`/`StatusBadge`/`JobTypeBadge`/`ProgressBadge`, `Card`/`GlassCard`/`SimpleCard`, `Tooltip`, `CoverFan`, `Button`/`IconButton`, `Input`/`Select`/`Textarea`/`SearchInput`, `Form`, `StatBox`, `Toast`, `Icon`) ;
  - routes/app : `app/health` (route handler) et `app/login` (formulaire).
- Le test de `lib/i18n/context` appelle `vi.unmock("@/lib/i18n/context")` pour rétablir le vrai provider (le setup le mocke globalement).
- Les tests de `lib/session.ts` utilisent `// @vitest-environment node` (`jose` compare les `Uint8Array` par realm, incompatible avec jsdom).
- Importer explicitement `describe`/`it`/`expect`/`vi` depuis `vitest` (pas de globals).

## Tests E2E (Playwright)

Smoke tests bout-en-bout dans `tests/e2e/`. Ils nécessitent l'API (7080), l'indexer et la base Postgres démarrés (la stack tourne sous Docker), et lisent `ADMIN_USERNAME`/`ADMIN_PASSWORD` depuis `.env.local`.

```bash
npm run test:e2e         # headless, réutilise le dev server sur 7082 s'il tourne
npm run test:e2e:headed  # avec navigateur visible
npm run test:e2e:ui      # mode UI Playwright
```

- `helpers.ts` expose `openFirstSeries(page)` (partagé par les specs).
- `auth.setup.ts` se connecte via `/api/auth/login` et enregistre la session dans `tests/e2e/.auth/state.json` (ignoré par git). Il ajoute aussi un cookie `as_user_id` pour rendre les UI « utilisateur actif ».
- `smoke.spec.ts` vérifie que chaque page se rend (HTTP < 400, `main`/`header` visibles, zéro `pageerror`).
- `interactions.spec.ts` couvre `ui/Modal` (ouverture/Escape/bouton), `DeleteConfirmButton`, `MarkReadButton`, `TestConnectionButton`, `JobsIndicator`.
- `flows.spec.ts` couvre la recherche de séries (`?q=`), l'ajout/retrait d'un tag dans le formulaire d'édition (sans enregistrer) et l'ouverture d'une modale via le menu d'actions.
- `flows-business.spec.ts` couvre liste → détail pour les livres et les jobs, les filtres de `/downloads`, la navigation `/libraries` → séries et la navigation par onglets de `/settings` (jusqu'au formulaire de tokens).
- Tests conditionnels : `test.skip(await locator.count() === 0, "raison")` quand une donnée ou une fonctionnalité n'est pas configurée (ex. provider de statut de lecture, base vide).
- Cibler un test : `npx playwright test smoke.spec.ts -g "series"`. Débug : `npx playwright test --debug`.

## Gotchas

- **Port 7082** : pas le port Next.js par défaut (3000). Défini dans `package.json` scripts (`-p 7082`).
- **API_BASE_URL** : en prod, configuré via env. En dev local, l'API doit tourner sur `http://localhost:7080`.
- **React 19 + Next.js 16** : utiliser les nouvelles APIs (actions serveur, `use()` hook) si disponibles.
- **Pas de gestion d'état global** : fetch direct depuis les Server Components ou `useState`/`useEffect` dans les Client Components.
