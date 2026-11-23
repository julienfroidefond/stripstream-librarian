# Un store d'écriture unique pour les recherches Telegram et DDL

Les deux fenêtres de recherche d'acquisition — la recherche live Telegram
(`app/components/TelegramSearchModal.tsx`) et la recherche chez les fournisseurs directs
(`app/components/DdlProviderSearchModal.tsx`) — agissent sur chaque ligne de résultat : lancer (ou
relancer) son téléchargement, ou l'ignorer. Chacune rejouait le même protocole à la main : un `Set`
d'identifiants en vol par verbe (`downloadingIds`, `dismissingIds`, `actingKeys`), un statut
optimiste écrit dans les lignes de résultat, un `prevStatus` capturé avant l'appel pour le restaurer
en cas d'échec, et un `catch {}` qui avalait silencieusement un *dismiss* refusé. Les lignes sont un
**read model** possédé par la modale : y écrire le badge optimiste faisait dépendre le rollback de
l'objet de ligne capturé par la fermeture, et les deux modales avaient déjà divergé (Telegram
fusionnait le statut depuis la ligne, DDL depuis une `Record<string, string>` séparée).

`lib/downloads/searchActions.ts` (pur, sans React) porte désormais ce protocole :

- un vocabulaire (`SearchResultAction` : `source` × `verb`) limité aux deux surfaces de recherche
  (`telegram`, `ddl`) et aux deux verbes d'une ligne (`start`, `dismiss`), avec
  `optimisticStatus(verb)` (`queued` / `dismissed`) comme seule dérivation du badge ;
- une identité de ligne, `searchKey(provider, external_id)` → `provider:external_id` : le même
  `external_id` trouvé sur deux fournisseurs reste deux lignes, et `externalIdOf(key)` sépare la
  moitié adresse ;
- **une seule table de routes** : le store résout `(source, verb)` via `actionFor` de
  `lib/downloads/actions.ts`, la table de la page Téléchargements. Il n'existe pas de second
  routage à maintenir en parallèle ; `telegram.start` et `telegram.retry` partagent déjà
  `downloadBook`, `telegram.dismiss` est la suppression douce, `ddl.start` snatche la release et
  `ddl.dismiss` retire la ligne de téléchargement ;
- une clé nomme une **release** (`provider:external_id`), pas toujours l'adresse de l'endpoint :
  `SearchActionContext` porte donc les données propres à l'appel — `ddlStart` (le fournisseur et le
  corps que la release snatche) et `downloadId` (la ligne de téléchargement qu'un *dismiss* DDL
  retire). Un *dismiss* DDL sans `downloadId`, ou un `start` DDL sans `ddlStart`, échoue
  explicitement au lieu d'appeler la mauvaise route ;
- un **overlay optimiste** (`status(key)`) écrit de façon synchrone à l'appel de `run`, avant même
  que la requête ne démarre. Un succès le conserve : la lecture serveur peut être en retard, et
  c'est `clear`/`reset` qui le retire, jamais le succès lui-même ;
- une **file par clé** : une rafale sur une même ligne exécute une requête à la fois (la dernière
  intention gagne le badge), deux lignes distinctes partent en parallèle. `pending(key)` reste vrai
  jusqu'au règlement de toutes les écritures de la clé ;
- une **seule politique d'échec** : l'overlay revient au dernier badge acquitté par le serveur (ou
  disparaît s'il n'y en a pas), l'erreur remonte à `onError` et la file de la clé continue. Un
  jeton par écriture protège le rollback : l'échec d'une écriture ancienne ne peut pas effacer
  l'overlay plus récent d'une écriture déjà en file ;
- `clear(keys)` retire les overlays qu'une lecture serveur fraîche supersède (le poll DDL) et
  **ignore les clés encore en vol** ; `reset()` vide tout quand une nouvelle recherche remplace les
  lignes — les écritures en vol finissent et gardent `pending` vrai, mais n'écrivent plus d'état ;
  `dispose()` coupe les listeners.

Le store ne fabrique jamais de ligne de résultat et ne relit jamais la recherche.

`app/components/useSearchActions.ts` monte un store par modale (`useSyncExternalStore`) et branche
`onError` sur le toast existant via `browserFailureMessage`, déplacé de
`app/(app)/downloads/useDownloadActions.tsx` vers `lib/api/browser/transport.ts` pour que les deux
stores partagent la même politique de message (détail upstream, `Error N` pour un HTTP sans détail,
message brut pour une erreur réseau). `useTelegramSearchActions()` et `useDdlSearchActions()` sont
les deux seules portes.

Les modales sont des adaptateurs : plus de `Set` en vol, plus de `prevStatus`, plus de `catch {}`.
Chaque ligne lit `status = actions.status(key) ?? statutServeur` et
`isActing = actions.pending(key)`. Le *retry* Telegram et le téléchargement utilisent le même verbe
`start`. La modale DDL garde son poll de 3 s : `hasActive` est calculé sur le statut fusionné
(overlay puis lecture serveur), et `refreshStatuses` appelle `actions.clear(...)` sur les clés que
le serveur connaît. `doSearch` et `handleClose` appellent `actions.reset()`. Les confirmations
restent l'affaire de l'adaptateur : les lignes de recherche agissent immédiatement, le store ne
demande jamais rien — y compris pour `ddl.dismiss`, dont la route de la page Téléchargements est
pourtant marquée `confirm`.

## Conséquences

- Lancer et ignorer un résultat depuis l'une ou l'autre recherche partagent une seule
  implémentation : badge optimiste, sérialisation par ligne, rollback et remontée d'erreur ne
  peuvent plus diverger entre les deux modales.
- Un *dismiss* refusé n'est plus silencieux : le toast s'affiche, l'overlay est retiré et la ligne
  retrouve son statut serveur (ou le dernier badge acquitté).
- Le badge optimiste ne dépend plus de l'objet de ligne capturé : une lecture serveur tardive ne
  peut pas annuler un rollback, et une écriture ancienne ne peut pas écraser un badge plus récent.
- Le poll DDL reste inchangé côté utilisateur, mais il ne peut plus retirer l'overlay d'une
  écriture en cours.
- Le protocole est couvert par `tests/unit/lib/downloads/searchActions.test.ts` avec une table de
  routes injectée (sérialisation, parallélisme, overlay synchrone, rollback vers l'acquitté,
  échec de *dismiss* remonté, clés namespacées, garde de jeton, `clear`/`reset`) ; les tests de
  composants des deux modales gardent les parcours visibles, et aucun sélecteur E2E n'a changé.
- `lib/downloads/actions.ts` reste la seule table de routes d'acquisition : une nouvelle cible ou
  un nouveau verbe s'y ajoute, pas dans le store de recherche.
