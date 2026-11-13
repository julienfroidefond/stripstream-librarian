# Les transitions de `analysis_status` dans un seul module

L'ADR 0004 a introduit `books.analysis_status`, mais ses règles restaient dispersées. On trouvait du SQL littéral dans `batch.rs` (claim, page counts, miniatures, statuts), dans `analyzer.rs` (filtres de sélection, régénération) et dans `api/books/mod.rs` (miniature rendue à la volée). Le type `AnalysisStatus` de `crates/core` exposait en plus des prédicats (`is_analysis_candidate`, `is_thumbnail_candidate`, `after_extraction`, `is_pending`) qu'aucun code n'appelait.

`stripstream_core::analysis` devient le seul écrivain de la colonne en dehors de la discovery, qui n'insère que `discovered` dans `ScanBatch`. Il expose `AnalysisMode::{Full, CoverOnly}` (les états sélectionnés, liés en `text[]`), `claim`, `release`, `mark_failed`, `record_page_counts`, `record_covers` (indexer), `record_cover` (API) et `reset_covers`. Les prédicats inutilisés sont supprimés. L'analyzer se contente d'orienter chaque résultat d'extraction vers la transition correspondante. Le choix entre `complete` et `cover_pending` après un page count reste en SQL, dans le module, parce qu'il dépend de la miniature présente au moment de l'écriture : l'API a pu la rendre entre-temps.

## Conséquences

- `release` ne remet en `discovered` qu'un livre `analyzing`. Une tâche de miniatures seules annulée laisse donc ses livres `cover_pending`, alors qu'elle les renvoyait auparavant en `discovered`.
- La régénération remet d'abord les lignes en `cover_pending` (`UPDATE … RETURNING`), puis purge les fichiers. Un arrêt entre les deux étapes ne laisse que des fichiers orphelins, jamais une ligne qui pointe vers une couverture supprimée.
- La progression de la phase de miniatures compte les couvertures enregistrées. Avant, elle relisait un vecteur déjà vidé par le flush et comptait 0.
- Chaque transition est couverte par un test sqlx dans `crates/core`.
