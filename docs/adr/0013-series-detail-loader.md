# Un loader de détail de série à dégradation unique

La page détail d'une série faisait dix-huit fetchs avec quatre politiques de repli inline, puis
passait le résultat à une toolbar de vingt-neuf props. `lib/series/detail.ts` (serveur, sans React)
porte tout le fan-out et renvoie un résultat explicite `{ status: "ok", model }` ou
`{ status: "not-found" }` ; la page ne fait plus que de la composition et la toolbar reçoit un
unique `model`. La dégradation a deux niveaux seulement : **fatal** (la série et sa bibliothèque,
sans lesquelles il n'y a rien à afficher) et **non fatal** (valeur vide par défaut + nom de la
section dans `model.degraded`).

## Conséquences

- Une panne de `/books` laisse la page en ligne avec une grille vide, comme avant, au lieu de
  transformer une série existante en 404 : un incident passager ne doit pas mentir sur l'existence
  de la série.
- Une section dégradée est nommée dans le modèle, donc diagnosticable, au lieu de disparaître
  silencieusement.
- Ajouter une section à la page signifie l'ajouter au loader et, si elle peut manquer, à
  `degraded` — jamais un `catch` local dans le JSX.
