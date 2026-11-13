# Une seule politique de fraîcheur du cache Next dans le backoffice

Les lecteurs de `lib/api` déclaraient leurs tags en littéraux (`"series"`, `` `series:${id}` ``,
`"metadata"`…) et chaque route handler ou server action invalidait à la main, avec des écarts :
`PATCH /api/series/[id]` expirait quatre entrées mais `DELETE` seulement `/series`, et la fusion,
la création de série, le renommage des livres, les genres, l'édition d'un livre et les tags de
bibliothèque n'invalidaient rien. `lib/readingListsCache.ts` était le seul helper de domaine.

`lib/cache/` porte désormais les deux côtés :

- `cacheTags` : les fabriques de tags utilisées par **tous** les lecteurs (`next: { tags }`) ;
- `invalidateAfter(write: DomainWrite)` : une union discriminée d'événements de domaine
  (`series.updated`, `series.deleted`, `series.merged`, `series.created`, `book.updated`,
  `readingProgress.changed`, `genres.changed`, `library.updated`, `readingLists.changed`,
  `metadata.changed`, `jobs.started`…). `invalidationPlan` (pur) traduit l'événement en tags et
  chemins ; `invalidateAfter` expire les tags avec `{ expire: 0 }` et revalide les chemins. Le
  contournement dev des listes de lecture (expirer aussi `/series` et `/reading-lists`) y vit.

Les routes et server actions disent *ce qui s'est passé*, jamais *quoi expirer*.

## Conséquences

- Un test de garde (`tests/unit/lib/cache/guard.test.ts`) et une règle ESLint
  (`no-restricted-imports`) interdisent `revalidateTag` / `revalidatePath` / `updateTag` et les
  tags littéraux hors de `lib/cache`. Seule exception tracée à l'origine, `TokensTab` a depuis
  été migré (`tokens.changed`, `users.changed`) : la liste d'exceptions est vide.
- Les server actions de `app/actions/cache.ts` passent de `updateTag` à
  `revalidateTag(tag, { expire: 0 })`, de sémantique équivalente (la requête suivante attend des
  données fraîches) et utilisable partout.
- Les `router.refresh()` côté client restent : un route handler qui invalide ne rafraîchit pas le
  routeur du navigateur, le refresh reste nécessaire pour re-rendre les Server Components.
- Ajouter une écriture = choisir (ou ajouter) un événement et sa ligne dans le test table-driven.
