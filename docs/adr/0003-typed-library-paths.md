# Des chemins de bibliothèque typés

Pour rendre explicite la traduction entre stockage et système de fichiers, `LibraryRoot` est construit au démarrage depuis `LIBRARIES_ROOT_PATH` et porté par `AppState`, tandis que `LibraryPath` conserve les formes base et locale. Les conversions passent par `resolve`, `to_local` et `to_db` : un chemin relatif échoue avec `LibraryPathError`, tandis que `resolve` conserve les chemins hôte absolus hérités hors racine.

## Conséquences

- Le code qui traduit les chemins reçoit la racine au lieu de relire `LIBRARIES_ROOT_PATH` ou de joindre `/libraries` à la main.
- La base conserve les chemins sous `/libraries`, avec une tolérance pour les anciens chemins hôte absolus ; les conversions explicitement limitées à une racine rejettent les chemins hors de celle-ci.
