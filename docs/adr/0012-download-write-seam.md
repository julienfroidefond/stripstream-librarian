# Une seule porte d'écriture pour les lignes de téléchargement

Les quatre surfaces de `/downloads` réimplémentaient chacune le protocole d'écriture d'une ligne :
`retrying`/`deleting` locaux, confirmation ad hoc, routage du retry vers le bon endpoint,
`try/catch` local puis toast et refresh. Les trois lignes encodeaient trois contrats d'échec
différents — l'une rafraîchissait même quand l'écriture échouait, une autre avalait l'erreur dans
un `catch {}` vide. `lib/downloadActions.ts` porte désormais le vocabulaire et le routage
(`actionFor`), la politique de confirmation comme donnée, un état par ligne
(`idle | pending(verb)`) qui coalesce les clics d'une même ligne, et une seule politique d'échec :
l'erreur remonte au toast, la ligne redevient `idle`, et **seul un succès rafraîchit**. Les
composants ne font plus que rendre cet état via `useDownloadActions`.

## Conséquences

- Un échec ne recharge plus la liste : l'utilisateur voit l'erreur et la ligne reste à sa place,
  prête à être réessayée.
- Une rafale de clics sur une ligne donne une requête ; deux lignes distinctes partent en
  parallèle.
- `useDownloadActions` lève hors de `DownloadActionsProvider` : toute page ou test qui rend une
  ligne doit fournir le provider.
- La liste noire du backoffice n'est pas une action de ligne et reste hors du seam.
