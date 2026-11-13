# Un seul module de progression de transfert

Quatre flux (torrents importés, AllDebrid, DDL, Telegram) réimplémentaient la progression, le
débit et l'ETA, avec trois formules d'ETA, deux politiques de `file_size` et deux vocabulaires de
snapshot. `downloads::progress` possède le throttle (`PROGRESS_UPDATE_BYTES`), un
`TransferProgress` unique (`from_bytes` / `from_reported`), `TransferRate` (lissage + ETA), le trait
`ProgressSink` et un `Reporter` qui décide *quand* écrire ; les sinks `TorrentSink`, `DdlSink` et
`TelegramSink` sont les seuls écrivains des colonnes de progression de leur table.

## Conséquences

- **La taille n'est jamais effacée** : `None` et `Some(0)` (taille inconnue) conservent la valeur
  déjà stockée, seul un `Some(n > 0)` la remplace. Un poll qBittorrent qui renvoie `0` en cours de
  transfert ne doit donc pas vider `file_size` ni casser l'ETA et la barre de progression.
- `rate.rs` et `target::DownloadProgress` disparaissent : un flux qui a besoin de la progression
  choisit un sink, pas un format.
- Toute nouvelle destination de téléchargement ajoute un `ProgressSink`, pas une boucle de
  throttle ni une formule d'ETA.
