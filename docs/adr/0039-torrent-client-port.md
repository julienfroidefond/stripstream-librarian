# Le protocole qBittorrent derrière un port

Deux flux parlaient l'API Web de qBittorrent à la main. Le poller de `torrent_import.rs` lisait
`torrents/info`, résolvait un hash par catégorie et supprimait avec `torrents/delete` ; l'ajout de
`qbittorrent.rs` cherchait un torrent existant, créait la catégorie `sl-{id}` et postait le magnet
ou le `.torrent`. Chacun écrivait sa requête, son en-tête `Cookie: SID=…`, son décodage du JSON et
sa table d'états — au point que `qbittorrent.rs` entretenait sa propre copie de `QbTorrentEntry`.
La boucle de poll n'était par ailleurs exerçable qu'avec un vrai qBittorrent : la table de
décisions et l'ordre « supprimer le torrent, puis le dossier de staging » n'étaient couverts par
aucun test.

## Décisions

- **`TorrentClient` est le port** (`downloads/torrent_client.rs`) : torrents d'un ensemble de
  hashes, torrents d'une catégorie, hash unique d'une catégorie, suppression (avec ou sans
  fichiers), retrait d'une catégorie. C'est exactement ce que consomment le poller et le
  nettoyage ; rien de plus. L'ajout (`add_url`, `add_file`, `create_category`, `find_existing`)
  reste une méthode inhérente de l'adaptateur, parce que seul le handler d'ajout s'en sert et
  qu'aucun test de poll n'a besoin de le simuler.
- **`QbittorrentClient` est l'adaptateur** : URL de base, client HTTP et cookie `SID` vivent dans
  une seule structure. `connect(pool)` charge la configuration puis se connecte en 10 s, avec les
  préfixes d'erreur `qBittorrent config: ` / `qBittorrent login: ` que le diagnostic et les tests
  attendaient déjà. La configuration (`QBittorrentConfig`, `load_qbittorrent_config`) et
  `qbittorrent_login` déménagent avec l'adaptateur ; `qbittorrent.rs` les réexporte pour
  `diagnostics/probe.rs`.
- **`TorrentInfo` et `TorrentState` normalisent la réponse** : une entrée `torrents/info` devient
  un type unique (`dlspeed` → `speed`), et les quatre tables d'états (complété, pause, stall,
  échec) sont déplacées telles quelles. `state_kind()` est le seul classifieur ; `is_completed()`
  (libellé **ou** fraction à 1.0) ne sert qu'à l'adoption de l'ajout.
- **`decide(info, row, now, stall_timeout)` est le verdict pur d'un poll** : complété → compléter,
  pause → ne rien faire, échec → échouer maintenant, stall → échouer seulement si le marqueur est
  plus vieux que le timeout, sinon armer, en cours → suivre. Comme l'ancien poll, il ne regarde que
  le libellé d'état, jamais la fraction seule. `stall_expired` et `missing_hashes` restent purs,
  et l'horloge est injectée : la table entière est un test unitaire, sans serveur, base ni horloge.
- **La boucle est générique** : `poll_qbittorrent_downloads` vérifie d'abord qu'une ligne
  `downloading` existe (`has_pending_rows`), pour qu'un poll au repos n'exige pas de qBittorrent
  configuré, puis connecte et délègue à `poll_with_client(pool, root, &dyn TorrentClient, now)`.
  Le timeout de stall n'est lu qu'au premier torrent bloqué, comme avant.
- **Le nettoyage prend aussi le port** : `cleanup_sources(pool, ReleaseTarget, Option<&dyn
  TorrentClient>)` / `release_sources` réutilisent une connexion existante quand elle est fournie,
  sinon ils se connectent eux-mêmes — les trois appels de production passent `None`. L'ordre
  « supprimer le torrent, puis le dossier `sl-{id}` » et l'invariant « un torrent `qb_owned =
  FALSE` n'est jamais supprimé » sont désormais testés avec un `FakeTorrentClient`.
- **L'ancien `enum TorrentClient` du ledger devient `TorrentBackend`** : le nom revenait au port.
  Renommage mécanique (`torrent_ledger.rs`, `qbittorrent.rs`, `alldebrid.rs`, tests), aucune
  valeur ni contrainte modifiée ; l'[ADR 0038](./0038-torrent-ledger-create.md) est corrigé.
- **AllDebrid garde son poller et n'implémente pas le port** : son transfert est un magnet dont
  l'état est un `statusCode` numérique rapporté contre un `external_id` (pas un hash), il n'a ni
  catégorie ni `qb_owned`, il supprime le magnet dès que la copie locale est complète, et il porte
  un état de tentatives, de backoff et de concurrence que qBittorrent n'a pas. Un trait commun ne
  partagerait que le mot « torrent » ; ses décisions propres restent testées dans `alldebrid.rs`.
- **Trois corrections délibérées, sans changement de statut visible** :
  - un échec de `torrents/delete` est maintenant loggé (`warn!`) dans le poller, le nettoyage et
    l'ajout ; l'ancien poller ignorait la réponse et annonçait « Removed failed torrent » même
    quand qBittorrent refusait ;
  - quand un stall expire, la progression est reportée avant l'échec (comme le faisait
    `arm_stall`), puis le torrent est retiré ;
  - un hash résolu tardivement est écrit en base puis complété en mémoire, au lieu de recharger
    toutes les lignes ; le délai de grâce de 5 minutes et celui des hashes absents (2 minutes)
    sont inchangés.

## Conséquences

- Le protocole qBittorrent n'existe plus qu'à un endroit : changer un endpoint, un champ ou un
  cookie touche l'adaptateur et ses tests wiremock, jamais les flux.
- **Comportement visible inchangé** : mêmes statuts, mêmes libellés d'erreur (« state: … »,
  « stalled in qBittorrent for N min »), même grâce, même import déclenché à la complétion, même
  ordre de nettoyage. Seuls les échecs de suppression apparaissent désormais dans les logs.
- La table de décisions, `stall_expired` et `missing_hashes` sont des tests unitaires ; le poll
  (progression, complétion → import, stall expiré, échec, torrent disparu, hash tardif, réponse
  illisible) et le nettoyage tournent sur `FakeTorrentClient` ; l'adaptateur seul garde des tests
  wiremock.
- Les invariants de sécurité sont explicites : torrent possédé supprimé **avant** son dossier,
  refus ou qBittorrent injoignable → dossier conservé, torrent adopté stockant ses données dans
  `sl-{id}` laissé intact, torrent adopté `stalledUP` importé mais jamais supprimé.
- Un futur client de torrent n'implémente le port que s'il partage le modèle qBittorrent
  (hash, catégorie, suppression explicite) ; sinon il garde ses décisions et ne réutilise que le
  ledger.
