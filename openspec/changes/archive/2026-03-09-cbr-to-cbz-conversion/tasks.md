## 1. Migration DB

- [x] 1.1 Créer `infra/migrations/0013_add_book_id_to_index_jobs.sql` : ajouter colonne `book_id UUID NULL REFERENCES books(id) ON DELETE SET NULL` à `index_jobs`
- [x] 1.2 Mettre à jour `sqlx-data.json` / préparer les queries sqlx si nécessaire

## 2. Parsers — Fonction de conversion

- [x] 2.1 Ajouter `pub fn convert_cbr_to_cbz(cbr_path: &Path) -> Result<PathBuf>` dans `crates/parsers/src/lib.rs`
  - Extraire le CBR vers `tmp_dir` avec `unar`
  - Lister et trier les images extraites
  - Créer `{stem}.cbz.tmp` dans le dossier parent du CBR avec la `zip` crate
  - Vérifier que le CBZ contient le même nombre d'images que le CBR
  - Renommer `.cbz.tmp` → `.cbz`
  - Nettoyer `tmp_dir`
  - Retourner le chemin du CBZ créé
- [x] 2.2 Ajouter la gestion d'erreur : collision de fichier existant, échec unar, échec zip, décompte incorrect

## 3. API — Endpoint de conversion

- [x] 3.1 Dans `apps/api/src/books.rs`, ajouter le handler `POST /books/:id/convert`
  - Vérifier que le livre existe
  - Vérifier que `file_format == 'cbr'`
  - Résoudre le chemin physique avec `LIBRARIES_ROOT_PATH`
  - Vérifier qu'aucun fichier `{stem}.cbz` n'existe déjà
  - Insérer un job `cbr_to_cbz` avec `book_id` en DB
  - Retourner le job créé
- [x] 3.2 Enregistrer la route dans `apps/api/src/main.rs`
- [x] 3.3 Ajouter les annotations `#[utoipa::path]` pour OpenAPI
- [x] 3.4 Mettre à jour `IndexJobResponse` / `map_row` dans `index_jobs.rs` pour inclure `book_id` (le champ est nullable)

## 4. Indexer — Worker de conversion

- [x] 4.1 Créer `apps/indexer/src/converter.rs` avec la fonction `convert_book(job_id, book_id, pool, config)`
  - Lire le livre en DB (file_path, file_format)
  - Résoudre le chemin physique
  - Appeler `parsers::convert_cbr_to_cbz()`
  - Mettre à jour `books.file_path`, `books.file_format`, `books.updated_at` en DB
  - Supprimer le CBR (log warning si la suppression échoue, ne pas faire échouer le job)
  - Mettre à jour le statut du job (`success` ou `failed`)
- [x] 4.2 Dans `apps/indexer/src/worker.rs`, ajouter le dispatch pour le type `cbr_to_cbz`
- [x] 4.3 Ajouter `converter` dans `apps/indexer/src/lib.rs`

## 5. Backoffice — Bouton de conversion

- [x] 5.1 Créer `apps/backoffice/app/components/ConvertButton.tsx` (composant client)
  - Appelle `POST /books/:id/convert` via `apiFetch`
  - Affiche un état de chargement pendant l'appel
  - En succès : affiche un message avec lien vers le job créé
  - En erreur : affiche le message d'erreur de l'API
- [x] 5.2 Intégrer `ConvertButton` dans `apps/backoffice/app/books/[id]/page.tsx`
  - Visible uniquement si `book.file_format === 'cbr'`
  - Placer dans la section des métadonnées ou comme action en haut de page
- [x] 5.3 Mettre à jour `JobRow.tsx` pour afficher le type `cbr_to_cbz` correctement (label lisible)

## 6. Vérification

- [x] 6.1 `cargo build` passe sans erreurs ni warnings clippy
- [ ] 6.2 Tester manuellement la conversion d'un livre CBR depuis la page détail
- [ ] 6.3 Vérifier que le CBR est supprimé et que `file_format` en DB est bien `'cbz'`
- [ ] 6.4 Vérifier le cas d'erreur : tenter de convertir un livre CBZ → 409
- [ ] 6.5 Vérifier le cas d'erreur : CBZ déjà présent → 409
