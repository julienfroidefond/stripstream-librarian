# La couverture des tomes a une seule source de vérité

`SeriesCoverage` dans `crates/core/src/coverage.rs` définit la règle commune afin d'éviter les divergences entre consommateurs : un `regular` ou `integral` numéroté (`volume >= 1`) couvre son tome, un `regular` non numéroté couvre un tome indéterminé, tandis qu'un livre `integral` non numéroté complète la série. Les `hs`, `oneshot` et volumes `0` ne comptent jamais ; les adaptateurs SQL de l'API vivent dans `apps/api/src/coverage.rs`.

## Conséquences

- Tout nouveau calcul de couverture passe par `SeriesCoverage` et ses adaptateurs, sans réintroduire de `COUNT(*)` de couverture dans un handler.
- Les gaps de métadonnées peuvent utiliser le total du provider lorsque celui de la série n'est pas défini, via `missing_count_for_total`.
- Le nettoyage de l'indexeur utilise le même type que l'API.
