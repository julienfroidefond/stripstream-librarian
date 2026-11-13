# Une seule résolution de la cible d'import

`target::prepare_destination` et `import_pipeline::do_import` résolvaient chacun la destination d'une série et formaient un cycle, alors que le nom fourni par l'utilisateur peut devenir un composant de chemin. `downloads::library_target::resolve` choisit le dossier connu en base, puis un dossier existant correspondant sans tenir compte de la casse ni des accents, puis `<root_path>/<series_name>` ; il renvoie aussi, si disponible, le livre du plus haut volume comme référence de nommage. Le nom n'est validé que pour les chemins de repli : un match en base peut contenir `/`, tandis que `..`, `/`, `\` et NUL sont rejetés avant l'accès disque de recherche du dossier.

## Conséquences

- Tout nouveau flux d'import utilise ce résolveur : la validation de traversée ne dépend plus des call sites.
- Le cycle de modules entre `target` et `import_pipeline` disparaît.
