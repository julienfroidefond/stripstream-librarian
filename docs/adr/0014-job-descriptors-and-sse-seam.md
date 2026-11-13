# Un registre de descripteurs de job et une seule porte SSE

La connaissance d'un type de job était éclatée entre le catalogue serveur (utilisé seulement pour
`category` et `replayable`), une map locale `JOB_TYPE_INFO` à dix-sept entrées, neuf prédicats
`job.type === "…"` et des fetchs de rapport par type ; trois composants ouvraient leur propre
`EventSource` en contournant `useEventSource`. `lib/jobs/descriptors.ts` est le registre unique
(libellé, description, catégorie, rejouabilité, rapport, plan de cartes), fusionné avec le
catalogue serveur — qui reste autoritaire pour `category` et `replayable` quand il répond.
`lib/useJobStream.ts`, construit sur `lib/useEventSource.ts`, est la seule porte des flux SSE jobs.

## Conséquences

- Un type de job inconnu du registre retombe sur le type brut, la catégorie `other` et aucune
  carte de résultat : un job ajouté côté indexer s'affiche sans casser la page.
- Les rapports sont injectés via `loaders` (`lib/jobs/serverReports.ts`) : le registre reste pur
  et importable par les Client Components.
- Plus aucun composant n'instancie `EventSource` ; la reconnexion, le watchdog et la pause d'onglet
  sont acquis partout, y compris sur les pages qui les avaient perdus.
