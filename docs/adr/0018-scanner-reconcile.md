# Un fichier connu est réconcilié une seule fois par le scanner

La discovery traitait un fichier déjà indexé dans deux branches recopiées : dossier à mtime inchangé et cas normal. Chacune résolvait la série, appliquait le renommage et écrivait `UPDATE books SET title…` ligne à ligne, avec des politiques d'erreur, d'annulation, de progression et d'événements différentes. Désormais, les deux chemins observent le fichier de la même façon : stat, fingerprint, progression et annulation. Ils appellent ensuite la fonction pure `scanner::reconcile::reconcile(existing, parsed, Observation) -> Reconciliation` (`Noop`, `Correct`, `Reindex`), puis `scanner::writer::ScanWriter::apply`. Le writer porte le cache de séries, le `ScanBatch` (UNNEST, une transaction) et les événements ; les nouveaux fichiers passent par `ScanWriter::insert_new`.

## Conséquences

- Divergences corrigées dans le dossier inchangé :
  - une correction de titre, de volume ou de type émet maintenant `book_updated` ;
  - un échec de stat suit la politique du cas normal (avertissement et détection de l'épuisement de descripteurs) au lieu d'être avalé ;
  - ses fichiers comptent dans la progression et déclenchent la vérification d'annulation ;
  - le message d'un réindexage est identique à celui du cas normal.
- Les corrections sont groupées dans le batch (`BookCorrection`, `series_id` conservé si la série est absente) au lieu d'être écrites immédiatement : elles sont écrites au flush, à l'annulation ou en fin de scan.
- Inchangé : `scanned_files` ne compte toujours pas les fichiers des dossiers inchangés, et un fichier connu d'un dossier inchangé reste marqué vu même si son stat échoue.
- `reconcile` se teste par table sans base, `apply` et la discovery par des tests sqlx sur une vraie arborescence. Les tests qui rejouaient l'UPDATE à la main sont supprimés.
