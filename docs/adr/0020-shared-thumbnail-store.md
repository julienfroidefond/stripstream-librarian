# Un seul magasin de miniatures partagé par l'API et l'indexer

La configuration des miniatures avait cinq lecteurs aux sources divergentes : `IndexerConfig` lisait des variables `THUMBNAIL_*` que personne n'utilisait, l'indexer décodait le JSON `app_settings.thumbnail` à la main, l'API relisait seulement le répertoire (ou `THUMBNAIL_DIRECTORY` pour la protection du cache) et rendait ses miniatures à la volée en dur (300 px, qualité 80, format source). L'écriture était atomique côté API mais directe côté indexer, la correspondance format → extension existait trois fois et le décodeur JPEG rapide deux fois.

Le crate `crates/thumbnails` (`stripstream-thumbnails`) devient le seul propriétaire : `ThumbnailSettings::load(pool)` applique une précédence unique — le réglage `app_settings.thumbnail`, champ par champ, sur des valeurs par défaut en code (WebP, 300×400, qualité 80, `/data/thumbnails`) ; aucune variable d'environnement n'est lue. `ThumbnailStore` porte la disposition `<répertoire>/<book_id>.<ext>` (`path_for`, `book_id_of`), l'encodage, la publication atomique (fichier temporaire + rename), les fichiers `.raw` de l'indexer et les purges. `fast_jpeg_decode` y est partagé avec le rendu des pages de l'API. Un crate séparé évite d'imposer `image`/`webp` à `stripstream-core`.

## Conséquences

- Le réglage en base est éditable à chaud depuis le backoffice ; garder une seconde source (env) aurait rendu la valeur effective imprévisible, la migration `0009` créant toujours la ligne. Les variables `THUMBNAIL_*` sont supprimées de `IndexerConfig`.
- La miniature rendue à la volée par `GET /books/{id}/thumbnail` suit désormais le format, les dimensions et la qualité configurés, comme celles de l'indexer.
- `publish` supprime les miniatures du même livre dans un autre format : changer de format ne laisse plus de fichier obsolète.
- La protection de `cache.directory` couvre le répertoire de miniatures configuré et le répertoire par défaut.
- Les écritures de `analysis_status` restent dans l'API et l'indexer, inchangées.
