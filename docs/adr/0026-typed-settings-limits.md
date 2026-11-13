# Un schéma typé pour `app_settings.limits`

Le schéma de la ligne `limits` vivait à quatre endroits de l'API : la liste des champs validés de `settings.rs`, `load_concurrent_renders` et `load_dynamic_settings` dans `state.rs`, et `DownloadSettings::from_limits`. L'écriture refusait `0` puis chaque lecture refaisait un `max(1)` ou un `clamp`, avec des règles qui avaient divergé (le délai de blocage d'un téléchargement acceptait 1 s à l'écriture et le relevait à 10 s à la lecture). Le rechargement à chaud était une suite de `if key == …` et `concurrent_renders` n'était jamais rechargé : le sémaphore de rendu était construit au démarrage, alors que la documentation annonce des réglages appliqués à chaud.

`apps/api/src/settings/limits.rs` devient la seule définition : un `Spec` (nom, défaut, min, max) par champ et une structure `Limits` à champs `NonZero*`. `Limits::parse(&Value) -> Result<Limits, Vec<FieldError>>` sert de validation à l'écriture et rapporte chaque champ fautif ; `Limits::from_stored` / `Limits::load` appliquent la même règle à la lecture, un champ stocké invalide retombant sur son défaut. `DownloadSettings` et `DynamicSettings` dérivent de `Limits` sans relire le JSON. `settings::apply(&AppState, key)` est l'unique point de rechargement à chaud.

`concurrent_renders` devient rechargeable : `RenderLimit` enveloppe le sémaphore `tokio` (file FIFO conservée). Agrandir ajoute des permis ; réduire oublie les permis libres et note le reste comme une dette, payée par les prochains rendus qui se terminent. Aucun rendu en cours n'est interrompu. Un échange atomique de sémaphore a été écarté : pendant la transition, l'ancien et le nouveau plafond se seraient cumulés.

## Conséquences

- Une valeur stockée hors plage (ancienne ligne, édition manuelle) retombe sur le défaut au lieu d'être bornée ; l'écriture la refuse de toute façon.
- `download_stall_timeout_seconds` est refusé sous 10 s à l'écriture, comme le formulaire du backoffice le bornait déjà.
- L'indexer lit encore `timeout_seconds` et `concurrent_renders` à sa façon (`analyzer.rs`, défauts 120 s et 1–2) ; l'aligner sur `Limits` suppose de déplacer le schéma dans `stripstream-core`.
