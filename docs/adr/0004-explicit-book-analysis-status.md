# Un état d'analyse explicite pour chaque livre

Le cycle d'analyse était déduit de `page_count` et `thumbnail_path` nullables, avec des index partiels et des règles de reprise ambiguës. La migration `0141` introduit `books.analysis_status` (`discovered`, `analyzing`, `cover_pending`, `complete`, `failed`), renseigne les lignes existantes et remplace ces index par des index sur l'état : l'analyseur reprend `discovered`, `failed` et `analyzing`, les jobs de miniatures sélectionnent `cover_pending`, et les résultats sont enregistrés explicitement.

## Conséquences

- Les diagnostics distinguent analyse en attente, miniature manquante et analyse échouée ; le gap `no_cover` correspond à `cover_pending`.
- Une régénération de miniatures préserve les couvertures qu'elle ne peut pas reconstruire.
- `analyzing` est un état de travail valide : une analyse interrompue est reprise au passage suivant.
