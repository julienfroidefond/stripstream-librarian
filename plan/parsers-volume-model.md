# Unifier le modèle de volume dans `crates/parsers`

## Constat

`extract_volume`, `extract_volumes` et `clean_title` reconnaissent des
marqueurs similaires mais avec des implémentations distinctes. Un même nom de
fichier peut donc produire un tome, une plage ou un titre nettoyé légèrement
différents selon le consommateur.

## Périmètre

- Introduire une analyse interne unique d'un titre/fichier, capable de produire
  les occurrences de tome, les plages explicites et les portions à retirer du
  titre.
- Garder les API publiques actuelles pendant la migration :
  `extract_volume`, `extract_volumes` et `clean_title` deviennent des
  projections du modèle commun.
- Préserver les règles actuelles de limites de plages et d'évitement des
  numéros de version.

Sont hors périmètre : matching de série/release, type de volume HS/intégrale et
heuristiques propres aux noms de fichiers Telegram/Prowlarr.

## API cible

Créer un type interne, par exemple `ParsedVolumeMarkers`, contenant les tomes
explicites, les plages validées et les segments textuels reconnus. Il reste
privé tant qu'aucun appelant externe n'a besoin de cette granularité.

## Migration

1. Écrire une table de cas commune à partir des tests existants de
   `extract_volume`, `extract_volumes` et `clean_title`.
2. Implémenter le modèle interne en conservant les signatures publiques.
3. Faire basculer successivement les trois fonctions, sans changer leurs
   résultats attendus.
4. Simplifier les regex et boucles devenues redondantes.

## Tests et critères de fin

- Les tests existants de parsing restent verts sans adaptation de résultat.
- Ajouter des invariants : le tome unique doit appartenir aux tomes extraits ;
  nettoyer puis extraire ne doit pas créer de nouveau tome ; les plages ne
  doivent pas générer de doublons.
- Une seule source de vérité reconnaît `T`, `Tome`, `Vol`, `Volume`, `#`, les
  zéros initiaux et les plages.
