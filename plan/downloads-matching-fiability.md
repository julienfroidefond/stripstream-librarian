# Fiabiliser le matching des téléchargements

## Contexte

La page **Downloads** du serveur contient de nombreux faux positifs dans les
propositions Prowlarr et, potentiellement, Telegram. Le cas `Saga` (94
propositions Prowlarr) montre qu'un résultat peut être retenu grâce à un tome
commun, sans preuve suffisante qu'il appartient à la série affichée.

Les modifications concernent uniquement la qualification des propositions.
Elles ne changent ni les livres existants, ni les téléchargements déjà importés.

## Constats techniques

| Sujet | État actuel | Risque |
| --- | --- | --- |
| Détection Prowlarr | La recherche par série retient une release lorsqu'un tome extrait recouvre un tome manquant. Le titre de la release n'est pas ensuite validé contre la série. | Une release d'une autre série portant `T05` peut être proposée pour toute série ayant le T05 manquant. |
| Polling RSS | Le titre est comparé avec un simple `contains` après normalisation. | Les titres courts ou génériques (`Saga`, `Hotel`, `Heads`, `Corum`) correspondent trop largement. |
| Intégrales | Les mots `intégrale`, `integral`, `complet` et `complete` couvrent tous les tomes manquants. | Une intégrale étrangère peut être présentée comme solution complète. |
| Extraction de tomes | Le parseur accepte des marqueurs explicites mais aussi plusieurs formes de numéros nus. | Un numéro d'édition, de version ou de sous-titre peut être confondu avec un tome. |
| Persistance | Les releases précédentes sont fusionnées et gardées ; un scan sans résultat ne les supprime pas. | Les faux positifs et les résultats périmés s'accumulent. |
| Échecs | Une release est affichée en échec si elle recouvre un tome ayant échoué, pas si cette release précise a échoué. | Les alternatives valides paraissent à tort défaillantes. |
| Telegram | Les résultats d'une recherche sont enregistrés après extraction du nom et du tome du fichier. | Une réponse de recherche large peut remplir la base avec des éléments non liés. |
| Interface | Le résumé affiche surtout le nombre de sources, les tomes manquants et les seeders. | Il est impossible d'évaluer la qualité d'un match avant d'ouvrir puis télécharger la release. |

## Objectif

Ne proposer automatiquement au téléchargement que les releases qui ont :

1. une correspondance de série explicable ;
2. une correspondance de tome non ambiguë ;
3. été revues lors d'une détection récente.

Les résultats incertains restent consultables, mais ne doivent pas être
confondus avec des propositions sûres.

## Proposition de conception

### 1. Centraliser la qualification d'une release

Créer une fonction commune, utilisée par la détection Prowlarr, le polling RSS
et les chemins Telegram qui proposent des livres :

```text
qualifier(release, série, tomes_manquants) ->
  Rejet | Match { tomes, niveau_de_confiance, raisons }
```

L'ordre est impératif :

1. vérifier la série ;
2. extraire et vérifier les tomes ;
3. appliquer le cas particulier des intégrales ;
4. attribuer une confiance et les raisons du résultat.

Une simple intersection de numéros ne doit plus suffire.

### 2. Rendre le match de série strict mais maintenable

- Normaliser accents, séparateurs et casse, comme aujourd'hui.
- Comparer une phrase complète avec frontières de mots, plutôt qu'une simple
  sous-chaîne.
- Pour les titres courts ou génériques, demander une preuve supplémentaire :
  titre exact normalisé, alias connu, ou au moins deux tokens spécifiques.
- Prévoir des alias manuels par série pour les titres réellement publiés sous
  un nom différent.
- Garder une fonction unique pour éviter les divergences entre Prowlarr, RSS et
  Telegram.

Exemples attendus :

| Série locale | Release | Décision |
| --- | --- | --- |
| `Les Géants` | `Les Géants - 07 - Moon.cbz` | Accepter : nom complet + tome explicite. |
| `Saga` | `Autre Saga T05.cbz` | Incertain ou rejeter : titre générique insuffisant. |
| `One Piece` | `Naruto T05.cbz` | Rejeter : seul le tome correspond. |
| `XIII` | `XIII T02.cbz` | Accepter : titre exact + tome explicite. |

### 3. Classer la preuve du tome

Faire remonter la stratégie ayant permis l'extraction :

- `explicite` : `T05`, `Tome 05`, `Vol. 05`, `#05` ;
- `plage` : `T01-T10` ;
- `integrale` : mots-clés d'intégrale, après validation stricte de la série ;
- `ambigu` : numéro nu ou forme permissive.

Les niveaux `explicite`, `plage` et `integrale` peuvent être automatiques si la
série est sûre. Un tome `ambigu` est conservé uniquement comme proposition à
confirmer.

### 4. Ajouter un score et une explication de match

Enrichir `AvailableReleaseDto` avec :

- `match_confidence`: `high`, `review`, ou `rejected` ;
- `match_reasons`: par exemple `series exact`, `tome T05 explicite`,
  `plage T01-T10` ;
- `last_seen_at` ;
- un identifiant stable de release : GUID Prowlarr, URL d'information ou
  empreinte normalisée.

Une release rejetée n'est pas enregistrée dans `available_downloads`. Une
release `review` reste visible, mais l'action de téléchargement demande une
confirmation détaillée.

### 5. Réconcilier les releases stockées

À chaque scan :

- mettre à jour les releases revues ;
- conserver temporairement les releases absentes, avec `last_seen_at` inchangé ;
- masquer par défaut les releases non vues depuis une durée définie ;
- supprimer les releases expirées ou celles qui ne couvrent plus aucun tome
  encore manquant ;
- ne pas conserver indéfiniment une release qui échoue la requalification.

La migration doit également requalifier les données existantes ou, plus
simplement, vider les propositions historiques pour les reconstruire au prochain
scan. Cette seconde option est préférable : les résultats actuels sont connus
pour être peu fiables.

### 6. Distinguer l'échec d'une release de l'échec d'un tome

Associer les tentatives de téléchargement à l'identifiant stable de la release.
L'interface affichera alors :

- `cette release a échoué` si son téléchargement précis a échoué ;
- `un téléchargement antérieur pour T05 a échoué` comme information secondaire
  pour les alternatives.

### 7. Durcir Telegram

Lors d'une recherche Telegram :

- comparer le nom de série extrait du fichier avec la série demandée avant de
  persister une proposition exploitable ;
- conserver les résultats non concordants comme résultats bruts, sans les
  injecter dans `available` ;
- appliquer la même qualification série/tome que Prowlarr avant l'affichage sur
  Downloads.

## Évolution de l'interface Downloads

### Résumé d'une série

Remplacer le seul compteur `94 Prowlarr` par une synthèse de qualité :

```text
3 tomes manquants · 6 propositions sûres · 2 à vérifier · dernière vérification : il y a 4 min
```

Les résultats expirés sont exclus du compteur principal et visibles via un filtre.

### Ligne de release

Afficher, en plus du titre et des seeders :

- badge de confiance : `Sûr` ou `À vérifier` ;
- tomes couverts et type de preuve ;
- raisons du match dans une infobulle ou un panneau de détail ;
- date de dernière détection ;
- état d'échec exact de la release.

Le bouton de téléchargement direct reste disponible pour `Sûr`. Pour
`À vérifier`, il ouvre une confirmation reprenant ces informations.

## Plan d'implémentation

- [ ] Extraire les règles de normalisation, match de série et match de tome dans
  un module de qualification partagé.
- [ ] Couvrir les faux positifs connus par des tests unitaires : `Saga`, titre
  non lié avec même tome, intégrale non liée, titres courts, plages et numéros
  ambigus.
- [ ] Remplacer le filtrage volume seul dans la détection Prowlarr par la
  qualification complète.
- [ ] Remplacer le `contains` du polling RSS par la même qualification.
- [ ] Ajouter confiance, raisons, identifiant stable et dernière observation aux
  DTO et à la persistance.
- [ ] Réconcilier les résultats à chaque scan et purger/reconstruire les données
  historiques existantes.
- [ ] Identifier les échecs par release plutôt que par recouvrement de tomes.
- [ ] Appliquer la qualification aux propositions Telegram.
- [ ] Mettre à jour la page Downloads et ses traductions.
- [ ] Documenter le comportement utilisateur dans `apps/docs/`.

## Validation

Tests API ciblés :

- `cargo test -p api downloads::tests::prowlarr -- --test-threads=1`
- `cargo test -p api downloads::tests::detection -- --test-threads=1`
- tests ajoutés pour le chemin RSS et Telegram.

Vérification fonctionnelle après déploiement :

1. relancer une détection sur une bibliothèque de test ;
2. contrôler que `Saga` ne reçoit plus de releases provenant d'autres séries ;
3. vérifier qu'un résultat à tome commun mais titre différent est absent ;
4. vérifier qu'une intégrale valide couvre les tomes manquants uniquement après
   validation de son titre ;
5. vérifier que la liste ne conserve pas les anciennes releases non revues.

## Hors périmètre

- Modifier les livres ou métadonnées déjà indexés.
- Télécharger ou supprimer des torrents existants.
- Dédupliquer des séries distinctes portant le même nom.
