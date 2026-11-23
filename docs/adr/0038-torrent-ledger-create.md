# Une seule fabrique pour créer une ligne de `torrent_downloads`

Le ledger de l'[ADR 0010](./0010-torrent-downloads-ledger.md) avait une porte d'entrée par
client : trois `INSERT INTO torrent_downloads` écrits à la main. qBittorrent en avait deux — le
torrent déjà complet sur disque (`status = 'completed'`, `progress = 1`, `qb_owned = FALSE`
littéraux) et le torrent qu'il venait de confier au client, qui ne nommait que `qb_hash` et
`qb_owned` et laissait les défauts de la table fixer `status = 'downloading'` et
`progress = 0`. AllDebrid n'en avait qu'un, qui posait `client = 'alldebrid'` et **rien d'autre** :
ses lignes héritaient donc de `qb_owned = TRUE`, le défaut de la migration 0142.

Les conventions de naissance étaient réparties entre deux littéraux SQL et les défauts du schéma.
Changer un défaut déplaçait silencieusement une naissance ; `qb_owned` affirmait qu'AllDebrid
possédait un torrent qBittorrent ; et la seule manière de distinguer « inséré » de « course
perdue » était de lire `rows_affected()` chez chaque appelant, alors que seul AllDebrid a un
index d'unicité à perdre.

## Décisions

- **`create(pool, NewTorrentDownload) -> sqlx::Result<Option<Uuid>>`** est le seul `INSERT` de
  production sur la table. Il renvoie l'id créé, ou `None` quand l'insertion a perdu la course :
  l'index partiel `torrent_downloads_alldebrid_active_hash_key` détient déjà une ligne AllDebrid
  active pour ce hash. L'appelant ne lit plus jamais `rows_affected`.
- **Une naissance typée, `Birth`** :
  - `Queued` — Stripstream vient de remettre le torrent à son client : `downloading`,
    progression 0, torrent possédé. Un envoi `.torrent` n'a pas encore de hash, il sera retrouvé
    par sa catégorie `sl-{id}`.
  - `Downloading` — le client fait déjà tourner le transfert (magnet AllDebrid, torrent
    qBittorrent ajouté par l'utilisateur) : `downloading`, progression 0 que le prochain poll
    remplace, torrent adopté.
  - `CompletedAdopted { content_path }` — le client détient déjà les fichiers finis : `completed`,
    progression 1. Le chemin de contenu est **dans la variante**, donc une naissance `completed`
    sans chemin est irreprésentable.
- **`TorrentBackend::{Qbittorrent, Alldebrid}`** (nommé `TorrentClient` à l'écriture de cet
  ADR, renommé en `TorrentBackend` par l'[ADR 0039](./0039-torrent-client-port.md)) porte le
  `client` de la contrainte de la migration 0125 et la règle de propriété : `owns_qb_torrent`
  n'est vrai que pour `Qbittorrent` + `Queued`. Une ligne AllDebrid stocke donc `qb_owned = FALSE` — délibérément,
  aucun lecteur ne consulte cette colonne hors des lignes qBittorrent (`fail_qb_torrent`,
  `release_qb_torrent` dans `torrent_import.rs`).
- **`NewTorrentDownload::new(id, library_id, series_name, client, birth)`** fixe l'identité, la
  cible et la naissance ; les champs optionnels que le backend rapporte (`qb_hash`,
  `external_id`, `name`) et les options d'import (`expected_volumes`, `replace_existing`) se
  remplissent par syntaxe de mise à jour de structure. Le SQL est une constante statique : sqlx
  0.9 exige un `&'static str`.
- **Les deux appelants interprètent `None` selon leur client** : AllDebrid garde sa branche
  « course perdue » (supprimer le magnet en double, renvoyer la ligne gagnante) ; qBittorrent,
  qui n'a aucun garde d'unicité sur son propre id, traite `None` comme une erreur interne.
- **Les transitions ne bougent pas** : `mark_completed`, `begin_import`, `finish_import`, `fail`,
  `restart`, `record_progress` et `arm_stall` restent le seul vocabulaire des écritures
  ultérieures.

## Conséquences

- Ajouter un client ou une naissance est un bras de `match` dans la fabrique, pas un nouvel
  `INSERT` : les trois colonnes que les anciens inserts se disputaient (`status`, `progress`,
  `qb_owned`) ont une seule définition.
- Les lignes AllDebrid changent de valeur stockée pour `qb_owned` (`FALSE` au lieu du défaut
  `TRUE`) sans changement de comportement : la colonne n'est lue que pour qBittorrent.
- Comportement visible inchangé : mêmes statuts, mêmes progressions, mêmes transitions, même
  gestion de la course AllDebrid. Aucune migration n'est nécessaire, et les `INSERT` bruts des
  fixtures de test restent valides.
- Chaque naissance est couverte par un `#[sqlx::test]` (statut, progression, `qb_owned`, hash,
  chemin de contenu), plus la course perdue, la frontière de l'index actif et l'absence de garde
  côté qBittorrent.
- La fabrique est le point d'entrée naturel du port de transfert de l'
  [ADR 0039](./0039-torrent-client-port.md) : les deux clients partagent déjà la forme de leur
  ligne, seule la collecte du transfert diffère encore.
