# Les disponibilités de téléchargement se merge et se prunent au même endroit

`available_downloads` reçoit des releases par merge lors de la détection, du RSS et du DDL, et perd des volumes par prune après un scan, un import de torrents ou une normalisation ; ces écritures réparties entre API et indexeur risquaient de diverger sur les règles de compteur. `crates/core/src/availability.rs` centralise ces deux directions, le parcours du JSON et la dérivation de `missing_count` ; `PresentVolumes` représente les volumes issus de `SeriesCoverage` ou d'un import.

## Conséquences

- Un prune décrémente le compteur du nombre de volumes numérotés distincts réellement retirés d'au moins une release, jamais du nombre de fichiers importés ou de releases correspondantes.
- Une release dont la liste `matched_missing_volumes` devient vide est supprimée, et les entrées héritées de volume `0` sont normalisées.
- Un prune ne retire que les volumes représentés dans le JSON.
