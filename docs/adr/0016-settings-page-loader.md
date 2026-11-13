# Un loader pour la page `/settings`, des réglages illisibles en lecture seule

La page `/settings` faisait une dizaine de fetchs avec neuf replis inline : des valeurs inventées
affichées comme réelles (`/tmp/stripstream-image-cache`, `/data/thumbnails`), des `apiFetch` bruts
à côté de fetchers existants, des casts `as`, et un fetch direct vers l'indexer qui essayait trois
hôtes en dur. Surtout, `getSettings().catch(() => ({}))` amorçait le brouillon avec les défauts
comme valeurs acquittées : une sauvegarde envoyait alors une section complète et pouvait écraser
la configuration stockée. `lib/settings/page.ts` (serveur, sans React) porte tout le fan-out et
renvoie un `SettingsPageModel` ; la page ne fait que de la composition et `SettingsPage` reçoit
un unique `model`. Dégradation unique, sans section fatale : une section en échec vaut `null`
(réglages, statistiques, versions) ou une liste vide, et son nom est ajouté à `model.degraded`.
`settings === null` met le brouillon (`createSettingsDraft({ readOnly })`) en lecture seule.

## Conséquences

- Réglages illisibles ⇒ un bandeau l'annonce, les éditions sont ignorées et aucune écriture ne
  part (`save` rend `readonly`) : des défauts ne remplacent plus jamais une configuration non lue.
- Une statistique ou une version indisponible s'affiche comme telle, jamais comme un chemin ou un
  numéro plausible.
- L'indexer est joint via une seule variable, `INDEXER_BASE_URL` (défaut `http://indexer:7081`) ;
  hors Docker, il faut la renseigner.
- Ajouter une donnée à la page signifie l'ajouter au loader et à `SettingsSectionName`, jamais un
  `catch` local.
