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
```

## Gotchas

- **Port 7082** : pas le port Next.js par défaut (3000). Défini dans `package.json` scripts (`-p 7082`).
- **API_BASE_URL** : en prod, configuré via env. En dev local, l'API doit tourner sur `http://localhost:7080`.
- **React 19 + Next.js 16** : utiliser les nouvelles APIs (actions serveur, `use()` hook) si disponibles.
- **Pas de gestion d'état global** : fetch direct depuis les Server Components ou `useState`/`useEffect` dans les Client Components.
