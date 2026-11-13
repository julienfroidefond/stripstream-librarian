# Un store d'écriture unique pour les listes de lecture

Quatre surfaces écrivaient les listes de lecture (`ReadingListsClient`, `ReadingListDetailClient`,
`AddToReadingListModal`, `AiReadingListsModal`) : douze écritures, quatre politiques d'échec
(silence ×5, rollback sur une closure périmée `items` dans le reorder, 400 traité comme « déjà
ajouté », comptage + toast). Chaque surface gardait sa copie dans `useState(initial)`, donc le
`router.refresh()` qui suivait chaque écriture ne la resynchronisait pas, et `handleAdd`
fabriquait un `ReadingListSeriesDto` factice (`library_name: ""`) parce que l'API répond `204`.

`lib/readingLists/actions.ts` (pur, sans React, sur le modèle de l'[ADR 0012](./0012-download-write-seam.md))
porte désormais le protocole :

- un vocabulaire d'opérations (`create`, `rename`, `delete`, `add`, `remove`, `move`, `reorder`,
  `moveToList`) routé vers `browserApi.readingLists` ;
- les écritures sont **confirmées par le serveur** puis réconciliées par relecture (`detail()`
  pour une liste suivie, `list()` pour une vue d'ensemble suivie) — jamais d'objet inventé ;
- seul le reorder (`move`/`reorder`) est optimiste ; un échec restaure le dernier ordre acquitté
  par le serveur, lu dans le store, et seulement si aucun reorder plus récent n'est en file ;
- les écritures d'une même liste sont sérialisées ; une rafale de clics sur un même item
  (`opKey`) est coalescée ;
- une seule politique d'échec : l'état reste (ou redevient) celui du serveur, `onError` (toast)
  est appelé et `onSuccess` (`router.refresh()`) seulement si quelque chose a été appliqué.
  `runAll` traite un lot (propositions IA) comme une seule action : une réconciliation, au plus
  un toast d'erreur et un refresh.

`app/components/ReadingListsProvider.tsx` est l'adaptateur React (`ReadingListsProvider`,
`useReadingLists`). Les pages vue d'ensemble et détail montent un provider ; la modale IA partage
celui de la vue d'ensemble ; la modale « ajouter à une liste » utilise un store local.

## Conséquences

- Un échec n'est plus silencieux et ne rafraîchit rien ; seul le libellé du toast reste propre à
  la surface (`failureMessage`).
- `router.refresh()` ne sert plus qu'aux sections rendues côté serveur (listes de la page série) :
  l'état des surfaces vient du store.
- `GET /api/reading-lists` transmet désormais `series_id` : la modale « ajouter » s'en sert pour
  marquer les listes qui contiennent déjà la série, au lieu de traiter un `400` comme un succès.
- La création d'une liste reste hors réconciliation `detail()` : le DTO renvoyé par `POST` est
  une réponse serveur réelle.
