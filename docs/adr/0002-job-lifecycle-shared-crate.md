# Un cycle de vie de tâches partagé entre l'API et l'indexeur

Pour appliquer les mêmes règles aux tâches de l'API et de l'indexeur, `crates/jobs` centralise leur registre, leur réclamation par propriétaire avec `FOR UPDATE SKIP LOCKED`, leurs transitions `success`, `partial` ou `failed` selon les unités tentées et les erreurs, ainsi que la priorité de l'annulation. Le registre décrit propriétaire, catégorie, politique de bibliothèque, réclamation, relance, exclusivité et priorité ; la création reste dans le service qui planifie la tâche.

## Conséquences

- Le nettoyage des tâches périmées ou orphelines est limité au propriétaire ; la purge des tâches terminées est globale.
- Le registre reste aligné sur la contrainte SQL `index_jobs_type_check`, ce qu'un test vérifie ; `GET /index/job-types` expose le catalogue.
