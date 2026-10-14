# Audit design — Backoffice StripStream

Audit initial **en lecture seule** de `apps/backoffice`, puis correction par lots.
Cible : interface d'admin → on garde les principes *dashboard/app* (densité, lisibilité,
états explicites), pas les recettes marketing (parallax, scrollytelling, bento, glassmorphism
généralisé).

## Légende

- ✅ fait — ⏳ à faire — ⛔ hors périmètre (ne pas « corriger »)

## Lots

### Lot 1 — Bugs P0 ✅

| # | Problème | Fichier | Correctif |
|---|----------|---------|-----------|
| 1 | `dark:` suivait `prefers-color-scheme`, pas le toggle `next-themes` (10 usages) | `app/globals.css` | ajout `@custom-variant dark (&:where(.dark, .dark *))` |
| 2 | classe `glass-card` utilisée mais jamais définie | `app/globals.css` | surface givrée définie (`color-mix` + `backdrop-filter`) |
| 3 | `.badge-unread` / `.badge-completed` en CSS invalide (`hsl(var(--x) / .6)`, vestige v3) | `app/globals.css` | réécrits en `color-mix` |
| 4 | `text-success-foreground` inexistant | `app/globals.css` | token ajouté (aligné sur `destructive-foreground`) |
| 5 | `tailwind.config.js` mort (v4 ne le charge pas sans `@config`) | racine app | supprimé ; `AGENTS.md` mis à jour |

### Lot 2 — Palette ✅

- **298 utilitaires codés en dur sur 18 teintes** → 6 tokens sémantiques.
- **Fond décoratif** : 9 dégradés radial (cyan + vert + ambre + violet) sur `body::before`/`::after`
  → **2 halos** (`primary` + `highlight`) sur `body::before`, thème-aware, `::after` supprimé.
- **Nav** : 9 teintes par item → 5 tokens cohérents (voir ci-dessous).
- **`JobTypeBadge`** : 19 teintes brutes → 5 domaines, motif `bg-<token>-soft text-<token>`
  (contraste garanti dans les deux thèmes, l'ancien `bg-x/80 text-white` devenait illisible en dark).
- **Contraste** : `--color-warning-foreground` (sombre) + `--color-success-foreground` (clair/sombre).

#### Table de correspondance appliquée

| Teintes brutes | Token |
|---|---|
| `green` `emerald` `lime` | `success` |
| `amber` `yellow` `orange` | `warning` |
| `red` `rose` | `error` |
| `sky` `blue` | `info` |
| `cyan` `teal` | `primary` |
| `violet` `purple` `indigo` `fuchsia` `pink` | `highlight` |
| `slate` `gray` `zinc` `neutral` `stone` | `muted-foreground` / `border` / `muted` |

#### Tokens ajoutés (`@theme`)

```
info            info-soft            info-foreground
highlight       highlight-soft       highlight-foreground
warning-foreground                   success-foreground (dark override)
```

#### Nav (5 tokens)

`series`→warning · `books`→success · `authors`/`discovery`→highlight ·
`data` (genres + metadata)→info · `reading-lists`/`libraries`→primary

### Lot 3 — Typographie ✅

- **Pile de police** : `Inter` était déclaré en tête mais **jamais chargé** (aucun `next/font`,
  `@font-face` ni asset dans `public/`) → supprimé. Pile assumée :
  `Avenir Next, Segoe UI, system-ui, Noto Sans, sans-serif`.
  *(Alternative si cohérence cross-plateforme voulue : auto-héberger Inter via `next/font/local`
  + un `.woff2` dans `public/` — réversible en une ligne dans `--font-sans`.)*
- **`--font-display`** (Baskerville) défini mais utilisé **0 fois** → supprimé.
- **Échelle micro** : 149 tailles arbitraires (`text-[10px]` ×104, `text-[11px]` ×42,
  `text-[9px]` ×3) → tokens `--text-2xs` (11px) / `--text-3xs` (10px) avec `line-height` ;
  les 3 valeurs `9px` remontées à `10px` (lisibilité).
- **Chiffres tabulaires** : règle globale `table { font-variant-numeric: tabular-nums; }`
  (12 tables) plutôt qu'un ajout fichier par fichier.

### Lot 4 — États & a11y ✅

- **`CollapsibleNav.tsx` supprimé** : aucun import dans `app/`, `lib/` ni les tests → code mort.
  La suppression règle d'un coup le bouton 28×28 px, les `aria-label` anglais et le
  `if (collapsed) return null` (perte de contexte). *(Si le repli de nav revient un jour, il faudra
  le réimplémenter en conservant le contexte — pas en `return null`.)*
- **`theme-toggle.tsx`** : 5 libellés anglais codés en dur → i18n (`theme.label|light|dark|system|
  switchToLight|switchToDark`) via `useTranslation`.
- **`aria-label` / `title`** : plus aucun littéral dans `app/` (vérifié par grep). Les 4 restants
  passés en clés : `LogoutButton` (`auth.logout`), `UsernameEdit` (`common.edit`), `RatingStars`
  (`rating.remove|removeAria` + `rating.ariaValue|unrated` interpolés), `UserGenreRestrictions`
  (`users.restrictions|manageRestrictions|genres|noGenre`).
- **Cibles tactiles — décision documentée** : les boutons d'icône du header (`ThemeToggle`,
  `LogoutButton`, `JobsIndicator`, `DownloadsIndicator`) sont en 36×36 px dans un header `h-12`.
  36 px **satisfait déjà WCAG 2.2 AA** (2.5.8 exige 24 px) ; les 44 px visés initialement relèvent
  du niveau AAA (tactile) et imposeraient de revoir tout le header 48 px. Conservé en l'état
  (densité desktop assumée) ; à reconsidérer uniquement pour une refonte tactile de `MobileNav`.
- Tests mis à jour : `theme-toggle.test.tsx`, `RatingStars.test.tsx` (assertions sur les clés i18n).

### Lot 5 — Mise en page ✅

- **Primitive `Skeleton`** (`app/components/ui/Skeleton.tsx`, exportée par le barrel `ui/`) :
  base `animate-pulse rounded-md bg-muted` + `aria-hidden`, la forme vient de `className`.
  Remplace les 3 skeletons inline du dashboard (`app/(app)/page.tsx`, 6 recopies de la même
  recette) et les 18 `<div className="… bg-muted">` de `app/(app)/metadata/loading.tsx`
  (l'`animate-pulse` passe du conteneur à chaque bloc). Test : `tests/unit/components/ui-skeleton.test.tsx`.
  *Changement visuel assumé : les blocs du dashboard passent de `bg-card border` (fausse carte
  bordée) à `bg-muted` — un seul langage de skeleton, aligné sur shadcn.*
- **En-têtes de tableaux — un seul langage** : `text-xs font-semibold text-muted-foreground
  uppercase tracking-wider` est désormais la règle partout (le jobs table l'appliquait déjà ;
  `MetadataTable` via `HEAD_CELL` ; `GenresClient` portait le style sur le `<thead>`).
  Normalisés (la densité — `px-4 py-3` vs `p-2` — reste volontairement variable selon le contexte) :
  `ArchivesTable` (2 tables), `ReadingOverviewTab` (2 tables), `ProwlarrDiscoveryList`,
  `DownloadDetectionCards`, `RenameSeriesBooksModal`, `ProwlarrSearchModal`, `GenresClient`.
  Les en-têtes étaient en `font-medium` à la taille du corps (`text-sm`) → hiérarchie faible ;
  ils sont maintenant en micro-label.
- **Bug de contraste trouvé en relisant les tableaux (reliquat Lot 2)** : `text-white` posé sur
  `bg-<token>` cassait le **dark mode**, où `--color-primary|success|info|warning|error-foreground`
  basculent en sombre (le token est clair en dark). Remplacé par le `-foreground` correspondant
  dans `Button` (warning), `BookCard`, `MetadataTable`, `SeriesGrid`, `WishlistTab`, `DiscoveryCard`,
  `MonitoringForm`, `JobRow`, `EditBookForm`, `EditSeriesForm`, `JobsList`, `SeriesAddModal` ;
  `BookCard` (pastille « manquant » sur `bg-muted-foreground/80`) → `text-background`.
  Token `--color-error-foreground` ajouté (absent, light `hsl(210 40% 98%)` / dark `hsl(222 35% 10%)`).
  Les `text-white` restants sont des **overlays sur image/scrim noir** (légitimes, à conserver).

### Lot 6 — Omissions ✅

- **Lien « aller au contenu »** : ajouté en tête de `app/(app)/layout.tsx` (`sr-only` +
  `focus:not-sr-only`, cible `#main-content`) et `<main>` reçoit `id="main-content"` +
  `tabIndex={-1}` + `focus:outline-none`. Seul `<main>` de l'app (vérifié par grep).
- **404** : `app/not-found.tsx` (URL inconnue, sans chrome) et `app/(app)/not-found.tsx`
  (appels `notFound()` d'une route — 8 routes en font — qui gardent header/nav). Les deux
  rendent le même composant `app/components/NotFoundView.tsx` (serveur, i18n).
- **Boundary d'erreur** : `app/error.tsx` et `app/(app)/error.tsx` (Client Components)
  partagent `app/components/ErrorView.tsx` — titre, description, bouton **Réessayer**
  (`reset`), lien retour au tableau de bord, et la `digest` de Next affichée si présente.
  `console.error(error)` pour garder la trace côté client.
  *`global-error.tsx` volontairement omis : il ne se déclenche que si le `RootLayout`
  lui-même plante (layout trivial — providers de thème/locale), au prix d'un `<html>`
  dupliqué. À ajouter seulement si un bug de layout devient plausible.*
- **`loading.tsx` sur les routes lourdes** : `jobs` (tableau), `series` et `books` (grille)
  via un `app/components/PageSkeleton.tsx` (titre + barre de filtres + contenu, `aria-busy`
  + libellé `sr-only`). Même intention que le `metadata/loading.tsx` existant : la navigation
  client affiche un squelette immédiat au lieu de figer l'écran précédent.
- **Factorisation** : `buttonClasses()` extrait de `ui/Button.tsx` et exporté par le barrel
  (`ui/index.ts`) — évite de dupliquer la liste de classes pour les liens stylés
  (`<Link>` ne peut pas être un `<button>`). `Button` consomme désormais ce helper.
- Clés i18n ajoutées (fr + en, parité vérifiée) : `common.skipToContent`, `error.*` (5),
  `notFound.*` (3).
- Tests : `ErrorView` (4), `NotFoundView` (2), `PageSkeleton` (2), `ui-button`/`buttonClasses` (3).
  *Le skip link lui-même n'est pas couvert par un test unitaire : `(app)/layout.tsx` est une
  frontière d'intégration (cookies, server actions, 8 composants) — couvert par les smoke
  tests e2e qui vérifient que chaque page rend sans `pageerror`.*

### Lot 7 — Complétude i18n ✅

Dette détectée pendant le Lot 4 : plusieurs écrans ne passaient pas par `t()`, donc restaient en
français même en locale `en`.

| Fichier | Reste en dur (avant) |
|---|---|
| `app/(app)/settings/components/ArchivesTab.tsx` | onglet entier — **0 appel `t()`** |
| `app/(app)/settings/archives/ArchivesTable.tsx` | en-têtes + états vides — **0 appel `t()`** |
| `app/(app)/jobs/[id]/components/DownloadDetectionCards.tsx` | en-têtes de table (« Titre », « Indexer », « Catégories », « Taille », « Seeders », « Date ») |
| `app/(app)/anilist/callback/page.tsx` | « Connexion AniList en cours… », « Redirection vers les paramètres… » |

*(`SettingsPage.tsx` → « Français » : nom de langue, à laisser tel quel.)*

#### Clés ajoutées (fr + en, parité vérifiée par le test dédié)

- `settings.archives.*` (22) : `count` (pluriel explicite), `loadError`, `empty`, `emptyHint`,
  `detailError`, `noBooks`, `volume`/`volumeHs`, `col*` (14 colonnes).
- `jobDetail.rssColumn*` (6) : `Title`, `Indexer`, `Categories`, `Size`, `Seeders`, `Date`.
- `settings.anilistCallback*` (7) : `Connecting`, `Redirecting`, `BackToSettings`, `NoToken`,
  `SaveError`, `TokenSaved`, `Connected` (interpolation `{{username}}`).
- `common.unknownError`, `jobDetail.reviewNeeded` (`DownloadDetectionCards`).

#### Groupes de clés par surface (pas de mutualisation transversale)

Les en-têtes sont dupliqués volontairement par écran (`jobDetail.rssColumn*`,
`settings.archives.col*`) plutôt que réutilisés d'une page à l'autre : c'est le motif déjà en place
dans le repo (`prowlarr.*` vs `discovery.*`). Une clé partagée couplerait deux écrans dont la
formulation peut diverger (contexte, abréviation, largeur de colonne).

#### Libellés de tome — non unifiés (décision)

`ArchivesTable.tsx` et `ReadingOverviewTab.tsx` portent chacun leur `volumeLabel`, et **on ne les
fusionne pas** : ce ne sont pas les mêmes libellés.
- `ArchivesTable` : libellés **traduisibles** (`T{{volume}}`, `HS {{volume}}`, puis
  `volumeType.oneshot`/`volumeType.integral`), car c'est une vue d'archive détaillée.
- `ReadingOverviewTab` : libellés **compacts et neutres en langue** (`T1`, `HS 2`, `OS`, `Int 3`,
  `—`), pensés pour un tableau dense.

`settings.archives` **réutilise** les clés canoniques `volumeType.hs|oneshot|integral` au lieu de
créer des doublons. Effet de bord assumé sur les valeurs fr : « Hors-série » → « Hors-Série » et
« Intégrale » → « Intégrales » (alignement sur le reste de l'app) ; en est corrigé
(« Special Edition », « Omnibus »).

#### Gotcha : `t` n'est pas mémoïsé

`useTranslation()` recrée `t` à **chaque** rendu (`createTranslateFunction(dict)`), donc le mettre
dans les deps d'un `useEffect` provoquerait une boucle de fetch infinie. Deux contournements :

- `anilist/callback/page.tsx` : l'état stocke une **clé i18n** (`{ key, params }`), pas une chaîne
  rendue → l'effet reste en `[router]`.
- `ArchivesTab.tsx` : stocke un booléen `failed` (pas un message traduit) → deps `[]`.

`anilist/callback` : les erreurs inattendues ne remontent plus `e.message` brut, elles affichent
`common.unknownError` ; l'échec d'enregistrement est géré explicitement (plus de `throw` sentinelle).

#### Note de test : `useRouter()` doit être stable

Le mock partagé de `tests/setup.ts` renvoie un **nouvel** objet routeur à chaque appel. Un composant
dont l'effet dépend de `[router]` boucle alors à l'infini sous `act()`. Le vrai `useRouter()` de
Next.js est stable entre rendus, donc le composant est correct : c'est le test qui doit surcharger
le mock localement avec un routeur stable (`vi.hoisted`), comme le fait déjà
`DeleteBookButton.test.tsx`. Appliqué à `tests/unit/app/anilist/callback.test.tsx`.

Tests : `ArchivesTab` (5), `ArchivesTable` (4), `anilist/callback` (4),
`DownloadDetectionCards` (3).

Clés ajoutées au Lot 4 : `theme.*` (6), `rating.*` (4), `users.restrictions|manageRestrictions|genres|noGenre`.

### À conserver ⛔

Ombres teintées, texture de grain, fond off-black `hsl(222 35% 10%)` en dark, icônes SVG maison,
`:focus-visible` global, `Button` (focus ring + `active:scale`).

## Validation

```bash
cd apps/backoffice
npm run build        # compile le CSS + type-check
npm run test:unit    # 992 tests
npm run lint         # 0 erreur (14 warnings no-img-element préexistants)
npx tsc --noEmit
```

État : **Lots 1 → 7 ✅**. Modifications laissées **non commitées** pour revue.

Contrôle « plus aucune teinte brute » :

```bash
grep -rnE '\b(bg|text|border|ring|from|to|via|fill|stroke)-(slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-[0-9]{2,3}' \
  apps/backoffice/app apps/backoffice/hooks --include='*.tsx' --include='*.ts' --include='*.css'
```
