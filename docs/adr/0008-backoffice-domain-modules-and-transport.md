# Le backoffice : des modules de domaine purs et un seul transport navigateur

Les vues du backoffice répétaient les règles de téléchargement, de paramètres, de readiness et de communication API. Ces règles vivent dans les modules sans React `lib/downloads`, `lib/settingsDraft`, `lib/settingsSections` et `lib/acquisitionReadiness`, les pages serveur utilisent `CatalogQuery` pour leurs filtres, et les composants rendent l'état ; les fonctions typées par domaine sont agrégées sous `browserApi` au-dessus d'un transport same-origin unique qui gère méthode, corps, en-têtes, JSON, statut, enveloppe `{ error }` et `ApiError`.

## Conséquences

- Les Client Components n'importent pas de valeur runtime de `lib/api` serveur et n'appellent pas directement `fetch("/api/...")`.
- Les écritures de paramètres sont sérialisées et coalescées par clé ; un rollback ne remplace pas une édition plus récente.
- La readiness est calculée côté serveur et exposée par `AcquisitionReadinessProvider` ; une toolbar ne recharge pas les settings pour la déterminer.
- Le brouillon de `/settings` reste une exception de state local à la route, avec un moteur testable sans React.
