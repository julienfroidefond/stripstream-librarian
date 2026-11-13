# Passer le DTO (ou le view model) plutôt que ses champs

`LibraryActions` recevait douze props recopiées une à une depuis `LibraryDto`, et `EditSeriesForm`
quatorze props dépaquetées depuis le modèle de détail de série. Chaque champ ajouté au DTO
demandait de toucher la page, l'interface du composant et la déstructuration ; chaque instance de
`LibraryActions` rechargeait en plus la liste des fournisseurs de métadonnées (N requêtes pour N
bibliothèques), et son formulaire lançait quatre écritures parallèles non atomiques suivies d'un
`window.location.reload()`.

Un composant reçoit désormais l'objet que la page possède déjà : `<LibraryActions library
providers />` (fournisseurs chargés une fois par la page), `<EditSeriesForm model />`
(`SeriesActionsModel` ou la tranche `EditSeriesModel`). La traduction objet → valeurs de
formulaire et formulaire → écritures vit dans des modules purs (`lib/libraries/settings.ts`,
`lib/series/editForm.ts`), testés sans React.

## Conséquences

- `diffLibrarySettings(before, form)` ne produit une écriture que pour une section modifiée ;
  `applyLibraryWrites` les applique **séquentiellement** et s'arrête au premier échec (les
  écritures passées restent acquises, l'API n'offrant pas de transaction entre ces endpoints) :
  une seule politique d'échec, l'erreur s'affiche dans la modale et le routeur est rafraîchi si
  au moins une écriture a abouti.
- Plus de `window.location.reload()` : les route handlers invalident via `invalidateAfter`, le
  client appelle `router.refresh()` ([ADR 0021](./0021-backoffice-cache-invalidation.md)).
- Une donnée partagée par toutes les lignes d'une liste se charge dans la page serveur et se
  passe en prop, jamais dans un `useEffect` par instance.
