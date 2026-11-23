# Une politique d'exécution des providers, portée par `metadata_providers`

La façon d'appeler un provider de métadonnées était répartie dans ses appelants :

- `metadata/batch.rs` et `metadata/refresh.rs` tenaient chacun un booléen `sc_rate_limited`,
  comparaient le nom (`provider_name == "senscritique"`) et cherchaient un message dans l'erreur
  (`e.contains(RATE_LIMITED_ERROR)`) pour décider si le provider devait être sauté jusqu'à la fin
  du job ;
- `metadata/sync.rs` et `metadata/shared_sync.rs` choisissaient le matching selon le nom :
  `match_books` pour les providers historiques, `match_books_for_new_provider` pour BDTheque et
  BDphile ;
- le résolveur existait en deux versions : `resolve_provider` (strict) et
  `sync::resolve_link_provider_name`, qui retombait silencieusement sur Google Books.

Conséquences : ajouter un provider obligeait à toucher plusieurs flux ; la raison d'un échec
(limite de débit, incident passager, requête impossible) vivait dans une chaîne de caractères ;
reformuler le message cassait le disjoncteur ; la capacité de matching vivait dans le mauvais
module.

`metadata_providers::run` porte désormais la politique. Un adaptateur décrit *pourquoi* il n'a
pas répondu avec `ProviderError`; un `ProviderRun` dit *s'il faut encore l'appeler*. Aucun
appelant n'inspecte un message ni un nom de provider.

## Décisions

- **`ProviderError::{RateLimited, Transient, Fatal}`** est la seule classification. `Display`
  rend le message nu ; `From<String>` / `From<&str>` produisent `Fatal` (un message non classé
  n'ouvre jamais de bloc) ; `http_status` classe `429` et `5xx` en `Transient`, le reste en
  `Fatal`. Une erreur de transport (`reqwest`) est `Transient`.
- **Seul SensCritique produit `RateLimited`** : son `429` abandonne immédiatement, sans retry.
  Les autres adaptateurs classent `429`/`5xx` en `Transient` et n'ouvrent jamais le disjoncteur ;
  les scrapers BDTheque/BDphile gardent leur retry/backoff interne.
- **`ProviderRun` possède le disjoncteur** : `is_blocked`, `block_for`, `record`, et `call`, qui
  n'exécute pas la closure quand le provider est bloqué, enregistre une limite de débit une seule
  fois et la logge. Les blocs sont indexés par id canonique (`senscritique`, jamais
  `senscritique_bd`) et vivent le temps du run : un nouveau job repart propre. Un bloc ne concerne
  que le provider fautif, donc le provider secondaire continue d'être appelé quand le principal
  disjoncte (#150).
- **Sémantique conservée à l'identique** : le disjoncteur était déjà par provider et par job dans
  les deux flux (batch et refresh) ; ils étaient d'accord, il n'y a donc pas eu à arbitrer entre
  deux comportements. Le message « Skipped: … rate-limited earlier in this job » et le comptage
  des séries en erreur sont inchangés.
- **`MatchingPolicy::{Standard, HtmlScraper}`** est une méthode de trait avec valeur par défaut ;
  BDTheque et BDphile la surchargent. `shared_sync::match_books_with(policy, …)` est le seul point
  d'entrée ; `match_books` et `match_books_for_new_provider` disparaissent. Le matching est une
  capacité du provider, pas une décision de l'appelant.
- **Un seul résolveur** : `resolve_provider_with(name, UnknownProvider)` canonise puis applique la
  politique. `resolve_provider` reste `Reject` (recherche, batch, refresh) ;
  `resolve_link_provider` est `Fallback` et utilise l'unique constante
  `FALLBACK_PROVIDER = "google_books"`. Le repli historique des liens créés hors providers est
  désormais une politique explicite et nommée, plus un défaut silencieux.
- **ADR 0015 n'est pas rouvert** : `Score::of_titles`, `apply_book_count` et `is_auto_match` sont
  inchangés.

## Conséquences

- Ajouter un provider, un motif de blocage ou une classe d'erreur se fait dans
  `metadata_providers` ; les flux batch/refresh/sync n'ont plus de branche par nom ni de
  `contains` sur un message. `RATE_LIMITED_ERROR` est privé à SensCritique et ne sert plus que de
  message.
- La couture est testable sans réseau : un faux `MetadataProvider` couvre chaque variante
  d'erreur, le saut d'un provider bloqué sans appel, et la récupération sur un run neuf ;
  `search_with_provider` (batch) et `refresh_link` sont couverts en `#[sqlx::test]`, y compris un
  lien portant un alias de découverte bloqué via sa clé canonique.
- Comportement visible inchangé : mêmes providers sautés, mêmes messages de série, même repli
  Google Books pour les liens hérités.
