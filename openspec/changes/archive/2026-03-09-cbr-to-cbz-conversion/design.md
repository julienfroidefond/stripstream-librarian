## Context

Stripstream Librarian stocke des BDs/ebooks en CBR, CBZ et PDF. CBR (RAR) nécessite `unrar` (outil non-libre) pour toute opération. CBZ est un ZIP — format standard open-source, universellement supporté. Le parseur utilise déjà `unar` pour extraire les CBR et `zip` crate pour lire les CBZ. La conversion CBR→CBZ réutilise donc l'infrastructure existante.

Le système de jobs (`index_jobs`) est le mécanisme standard pour les opérations longues (rebuild, thumbnail). La conversion s'y intègre naturellement.

## Goals / Non-Goals

**Goals:**
- Convertir un livre CBR individuel en CBZ via un job asynchrone
- Garantir l'intégrité : le CBR n'est supprimé qu'après vérification du CBZ
- Exposer un endpoint API `POST /books/:id/convert`
- Afficher un bouton de conversion sur la page détail du livre (si CBR)
- Suivre la progression dans la liste des jobs existante

**Non-Goals:**
- Conversion batch (toute une bibliothèque) — déferred
- Conversion PDF→CBZ ou CBZ→CBR
- Re-indexation Meilisearch forcée après conversion
- Regénération du thumbnail après conversion (les images sont identiques)

## Decisions

### D1 : Stocker le `book_id` cible dans `index_jobs`

**Décision** : Ajouter une colonne `book_id UUID NULL` à `index_jobs` via migration.

**Alternatives considérées** :
- Stocker dans `stats_json` (champ JSON existant) → pas typé, query difficile
- Passer via un paramètre de route dans l'indexer → pas possible, l'indexer poll la DB

**Rationale** : Colonne typée, indexable, requêtable proprement. La migration est simple.

### D2 : Logique de conversion dans `crates/parsers`

**Décision** : Nouvelle fonction publique `convert_cbr_to_cbz(cbr_path: &Path) -> Result<PathBuf>` dans `crates/parsers`.

**Alternatives considérées** :
- Dans `apps/indexer/src/converter.rs` directement → moins réutilisable, difficile à tester isolément
- Appel shell `unar` + `zip` sans abstraction → identique à l'existant, cohérent

**Rationale** : Les parsers contiennent déjà toute la logique I/O archive. La fonction sera testable sans démarrer l'indexer.

### D3 : Fichier temporaire `.cbz.tmp` dans le même dossier

**Décision** : Créer `{stem}.cbz.tmp` dans le dossier parent du CBR, puis `rename()`.

**Alternatives considérées** :
- `tmp_dir` système → rename cross-device impossible (EXDEV), nécessite une copie
- Dossier temp dans le même filesystem → moins simple, idem pour le rename

**Rationale** : `rename()` sur le même filesystem est atomique. Même dossier garantit même device.

### D4 : Collision de nom → refus explicite

**Décision** : Si `{stem}.cbz` existe déjà → le job échoue avec une erreur claire, CBR intact.

**Alternatives considérées** :
- Écraser le CBZ existant → dangereux si l'utilisateur a édité le CBZ manuellement
- Renommer avec suffixe (`-converted.cbz`) → confusant pour la bibliothèque

**Rationale** : Fail-safe. L'utilisateur doit résoudre le conflit manuellement.

### D5 : Chemin DB avec LIBRARIES_ROOT_PATH remapping

**Décision** : Le chemin en DB commence par `/libraries/`. La conversion utilise le même remapping que l'indexer (`config.libraries_root_path`) pour résoudre le chemin physique. Après conversion, mettre à jour `file_path` (`.cbr` → `.cbz`) et `file_format` = `'cbz'`.

## Risks / Trade-offs

- **Chemin temp sur même device** → Si `LIBRARIES_ROOT_PATH` pointe vers un mount différent du `/tmp`, le rename sera cross-device. Mitigation : écrire `.cbz.tmp` dans le dossier parent du CBR, pas dans `/tmp`.
- **Processus concurrent** → Un job de rebuild lancé pendant la conversion pourrait scanner le fichier `.cbz.tmp`. Mitigation : le `.tmp` n'a pas d'extension reconnue (`detect_format` retourne `None`), il sera ignoré.
- **Fichier CBR ouvert par un lecteur** → Suppression du CBR possible sur Linux même si ouvert. Sur macOS/Windows, la suppression pourrait échouer. Mitigation : log l'erreur de suppression mais ne pas faire échouer le job (le CBZ est déjà valide et en DB).
- **Annulation du job** → Si le job est annulé pendant la conversion, le `.cbz.tmp` peut rester sur le disque. Mitigation : le worker nettoie les fichiers `.cbz.tmp` orphelins au démarrage (ou on documente le risque).

## Migration Plan

1. Appliquer `0013_add_book_id_to_index_jobs.sql`
2. Déployer API + Indexer simultanément (pas de breaking change — `book_id` est NULL pour les anciens types de jobs)
3. Rollback : la colonne `book_id` est nullable, les anciens jobs ne sont pas affectés
