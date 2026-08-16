# Centraliser le matching de titres dans `crates/parsers`

## Décision proposée

Créer un module `matching` dans le crate partagé `parsers`. Il devient la
source unique des règles pures permettant de qualifier un titre de release ou
de fichier par rapport à une série et à ses tomes manquants.

Les chemins Prowlarr, RSS et Telegram n'implémentent plus leurs propres règles
de normalisation, d'inclusion de titres et de correspondance de tomes. Ils
appellent le module partagé et conservent uniquement leurs responsabilités
métier : requêtes réseau, données locales, blacklist, persistance et interface.

## Pourquoi

Les règles sont aujourd'hui réparties entre :

- `apps/api/src/downloads/prowlarr.rs` : normalisation, inclusion de titre,
  intégrales et tomes manquants ;
- `apps/api/src/downloads/detection.rs` : qualification Prowlarr par
  recouvrement de tomes ;
- `apps/api/src/downloads/rss_poll.rs` : qualification RSS par inclusion de
  chaîne puis recouvrement de tomes ;
- `apps/api/src/downloads/telegram_monitor.rs` : déduction de série et tome
  depuis un fichier Telegram ;
- `apps/api/src/integrations/discovery/mod.rs` : déduction de série depuis un
  titre Prowlarr ;
- `crates/parsers/src/lib.rs` : extraction de volumes déjà partagée.

Cette dispersion produit des règles différentes selon la source. Elle rend
possible le faux positif actuel : une release Prowlarr est retenue pour un tome
commun sans vérifier assez fortement le nom de la série.

## Frontière du module

### Dans `parsers::matching`

Le module ne dépend ni de SQL, ni de HTTP, ni des DTO de l'API. Il reçoit des
chaînes et des numéros, et retourne un résultat explicable.

```rust
pub fn normalize_title(value: &str) -> String;

pub fn match_series_title(
    candidate_title: &str,
    series_name: &str,
) -> SeriesTitleMatch;

pub fn extract_volume_evidence(title: &str) -> VolumeEvidence;

pub fn match_release_title(
    candidate_title: &str,
    series_name: &str,
    missing_volumes: &[i32],
) -> ReleaseTitleMatch;
```

Types proposés :

```rust
pub enum MatchConfidence {
    High,
    Review,
    Reject,
}

pub enum VolumeEvidenceKind {
    Explicit, // T05, Tome 05, Vol. 05, #05
    Range,    // T01-T10
    Integral, // intégrale, complete, etc.
    Ambiguous,
}

pub struct ReleaseTitleMatch {
    pub confidence: MatchConfidence,
    pub matched_missing_volumes: Vec<i32>,
    pub all_volumes: Vec<i32>,
    pub series_match: SeriesTitleMatch,
    pub volume_evidence: VolumeEvidenceKind,
    pub reasons: Vec<MatchReason>,
}
```

Les raisons restent structurées afin que l'API puisse les traduire :
`SeriesExact`, `SeriesWordBoundary`, `ExplicitVolume(5)`, `Range(1, 10)`,
`Integral`, `GenericShortTitle` ou `NoSeriesMatch`.

### Hors de `parsers::matching`

Ces responsabilités restent dans `apps/api` :

- requêtes Prowlarr, RSS et Telegram ;
- séries et tomes manquants récupérés en base ;
- aliases administrables et décisions manuelles ;
- blacklist et durée de vie des releases ;
- persistance dans `available_downloads` ;
- seeders, téléchargement qBittorrent et état d'échec ;
- texte traduit et présentation Backoffice.

## Règles de matching

### Série

1. Normaliser casse, accents et séparateurs.
2. Accepter le nom de série comme une séquence complète de mots, jamais comme
   simple sous-chaîne non bornée.
3. Marquer les titres courts ou génériques comme `Review` quand ils ne disposent
   pas d'une preuve forte supplémentaire.
4. Rejeter une release sans correspondance de série, même si son tome est
   manquant localement.

### Tomes

1. Réutiliser `extract_volumes` mais exposer la preuve ayant permis l'extraction.
2. Accepter automatiquement les marqueurs explicites et les plages après un
   match de série fort.
3. N'accepter une intégrale qu'après un match de série fort ; elle couvre alors
   les tomes manquants.
4. Traiter les numéros nus ou les formes permissives comme `Review` plutôt que
   comme une preuve automatique.

## Consommateurs à migrer

| Consommateur | Migration |
| --- | --- |
| `downloads/detection.rs` | Remplacer le filtrage « tome manquant seulement » par `match_release_title`. |
| `downloads/rss_poll.rs` | Remplacer `title_matches_series` et le match de tome séparé par le même appel. |
| `downloads/prowlarr.rs` | Retirer normalisation, détection d'intégrale et `match_title_volumes` locaux au profit du crate. |
| `downloads/telegram_monitor.rs` | Réutiliser la normalisation et la qualification avant de proposer un fichier comme disponible. |
| `integrations/discovery/mod.rs` | Réutiliser `normalize_title` et le match de série strict pour annoter les résultats locaux. |
| `metadata/shared_sync.rs` | Réutiliser seulement `normalize_title` si pertinent ; sa règle de rapprochement de livres par volume reste métier. |

## Plan de migration

- [ ] Ajouter `crates/parsers/src/matching.rs` et exporter le module.
- [ ] Déplacer/réécrire les tests de normalisation et d'extraction de tomes
  nécessaires dans le crate partagé.
- [ ] Ajouter des tests de non-régression : `Saga`, titre étranger avec même
  tome, intégrale étrangère, plage valide, titre court et numéro ambigu.
- [ ] Migrer le chemin de détection Prowlarr.
- [ ] Migrer le polling RSS.
- [ ] Migrer Telegram et la découverte Prowlarr.
- [ ] Retirer les helpers devenus dupliqués dans l'API.
- [ ] Ajouter au DTO Downloads la confiance et les raisons structurées.
- [ ] Vérifier que les résultats existants sont requalifiés ou reconstruits.

## Validation

Le crate `parsers` porte la majorité des tests unitaires, sans PostgreSQL ni
services externes. L'API ajoute des tests ciblés pour vérifier que Prowlarr,
RSS et Telegram appellent tous la même qualification.

Critères :

- une release `Naruto T05` ne peut jamais être proposée pour `One Piece` ;
- une intégrale d'une autre série ne couvre aucun tome manquant ;
- `Les Géants - 07 - Moon.cbz` reste accepté pour `Les Géants` ;
- un résultat ambigu est identifiable et ne déclenche pas un téléchargement
  direct ;
- les trois flux donnent la même décision pour le même titre.
