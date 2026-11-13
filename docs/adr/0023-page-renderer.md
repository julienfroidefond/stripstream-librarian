# Un seul rendu de page et un seul « fichier courant » d'un livre

`books/pages.rs` recopiait trois fois la séquence cache mémoire → cache disque → verrou par clé → permis `page_render_limit` → timeout → `spawn_blocking` : dans `get_page`, dans le prefetch des pages suivantes et dans la miniature à la volée. Le format d'entrée venait de la base pour la page demandée mais de l'extension du fichier pour le prefetch. La requête « fichier courant d'un livre » (`book_files` le plus récent par `updated_at`) était écrite à la main une quinzaine de fois.

`books/renderer.rs` porte désormais la séquence. `PageRenderer::get(&PageSpec, Urgency)` sert une page (`Foreground` : attend un permis, remonte les erreurs, compte hits/misses ; `Prefetch` : ignore une page en cache, abandonne sans permis sous 100 ms, avale les erreurs) et `PageRenderer::run` expose le seul étage permis → timeout → `spawn_blocking` pour la miniature. L'accès aux archives passe par le trait `PageSource` : `ArchivePages` (adaptateur `parsers`) en production, un faux en test. `PageSpec` porte le chemin local, le `BookFormat` lu en base et la clé de cache, calculée une fois ; le prefetch réutilise le format de la page demandée.

`books/files.rs` porte la règle du fichier courant : `current_book_file(pool, &LibraryRoot, book_id) -> BookFile` pour les lectures unitaires (page, miniature, téléchargement), et la macro `current_book_file_join!("colonnes")` (le `LEFT JOIN LATERAL … bf` sur `books b`) pour les requêtes ensemblistes.

## Conséquences

- Le prefetch d'un fichier dont l'extension ne correspond pas au format indexé rend avec le bon format.
- Un `If-None-Match` correspondant est traité avant la lecture des caches : la réponse 304 porte toujours `ETag` et `Cache-Control`, et ne compte plus de hit mémoire.
- Les clés de cache disque des rendus non redimensionnés sont inchangées. Depuis la correction de la revue du 2026-11-13, le filtre de redimensionnement entre dans la clé (et l'ETag) des rendus redimensionnés : les anciennes entrées redimensionnées deviennent orphelines et restent sur disque jusqu'à « Vider le cache ».
- Les deux sommes `DISTINCT ON (book_id)` des statistiques, sans alias `b`, restent écrites à la main.
