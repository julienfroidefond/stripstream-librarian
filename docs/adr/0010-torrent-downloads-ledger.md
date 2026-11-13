# Le cycle de vie des torrents dans un ledger gardé

`torrent_downloads` était écrit depuis l'import, le poller qBittorrent, AllDebrid et les handlers,
avec des statuts en chaînes libres, des `UPDATE` sans garde sur le statut courant et des
transitions recopiées. `downloads::torrent_ledger` possède désormais le vocabulaire
(`TorrentStatus`), toutes les écritures de la table et les transitions (`mark_completed`,
`begin_import`, `finish_import`, `fail`, `restart`) ; chaque transition vérifie le statut courant
dans le `WHERE` de la requête unique et échoue avec `TorrentTransitionError` (`NotFound`,
`NotAllowed`, `UnknownStatus`, `Database`) au lieu de réécrire un état obsolète.

## Conséquences

- La colonne `status` reste `TEXT` : le typage est en Rust, les migrations et les vues SQL
  existantes ne bougent pas.
- Aucun `UPDATE torrent_downloads` hors du module ; les handlers convertissent l'erreur en
  `ApiError`, les pollers la journalisent et l'ignorent.
- Une transition perdue (statut déjà changé par un autre worker) est un refus explicite, pas un
  succès silencieux : les effets de bord (scan, événements, notification) ne se déclenchent qu'une
  fois.
