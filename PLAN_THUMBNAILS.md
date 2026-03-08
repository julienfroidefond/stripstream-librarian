# Plan: Génération des vignettes à l'index

## 1. Base de données

### Migration SQL (`0010_add_thumbnails.sql`)
- [x] Ajouter `thumbnail_path TEXT` à la table `books` (nullable)
- [x] Ajouter settings pour thumbnails dans `app_settings`:
  ```json
  {
    "thumbnail": {
      "enabled": true,
      "width": 300,
      "height": 400,
      "quality": 80,
      "format": "webp"
    }
  }
  ```

---

## 2. Configuration

### `crates/core/src/config.rs`
- [x] Ajouter `ThumbnailConfig` struct
- [x] Ajouter champs dans `IndexerConfig`:
  - `thumbnail_width: u32` (défaut: 300)
  - `thumbnail_height: u32` (défaut: 400)  
  - `thumbnail_quality: u8` (défaut: 80)
  - `thumbnail_dir: String` (défaut: `/data/thumbnails`)
- [x] Ajouter getter depuis env vars

---

## 3. Indexer - Extraction de la 1ère page

### Fonction à créer dans `crates/parsers/src/lib.rs`
- [x] `extract_first_page(path: &Path, format: BookFormat) -> Result<Vec<u8>>`
  - Réutiliser logique de `pages.rs:extract_cbz_page`
  - Réutiliser logique de `pages.rs:extract_cbr_page`
  - Réutiliser logique de `pages.rs:render_pdf_page`

### Fonction de génération vignette dans `apps/indexer/src/main.rs`
- [x] `generate_thumbnail(image_bytes: &[u8], config: &ThumbnailConfig) -> Result<Vec<u8>>`
  - Load image avec `image::load_from_memory`
  - Resize avec `image::resize` (ratio kept)
  - Encode en WebP avec `webp::Encoder`

- [x] `save_thumbnail(book_id: Uuid, thumbnail_bytes: &[u8], config: &ThumbnailConfig) -> Result<String>`

### Intégration dans `scan_library`
- [x] Après parsing metadata, extraire 1ère page
- [x] Générer vignette et sauvegarder
- [x] Stocker chemin en DB (via batch insert)

---

## 4. Indexer - WalkDir parallèle

### Remplacement de `WalkDir` séquentiel
- [x] Utiliser `rayon` pour paralléliser le scan:
  ```rust
  let total_files: usize = library_paths.par_iter()
      .map(|root_path| { ... })
      .sum();
  ```
- [x] Ajouter `rayon = "1.10"` dans workspace dependencies

---

## 5. API - Service des vignettes

### Mise à jour models dans `apps/api/src/books.rs`
- [x] Ajouter `thumbnail_url: Option<String>` à `BookItem`
- [x] Ajouter `thumbnail_url: Option<String>` à `BookDetails`
- [x] Mise à jour des requêtes SQL pour récupérer `thumbnail_path`

### Nouvelle route dans `apps/api/src/main.rs`
- [x] Route `/books/:id/thumbnail` (GET)
  - Retourne fichier statique depuis `thumbnail_path`
  - Content-Type: image/webp
  - Cache-Control: public, max-age=31536000

### Suppression cache 1ère page (optionnel)
- [ ] Optionnel: simplifier `pages.rs` car thumbnail pré-générée
- [ ] Garder render pour pages > 1

### Adapter backoffice

La recupération des thumbnail est fait par une route page/1. 
- [x] Passer par la nouvelle route avec une route clean /thumbnail pour chaque cover.

### refacto code entre api et indexer 

En fait l'indexer pourrait appeler l'api pour qu'il fasse les vignettes et c'est l'api qui est responsable des images et des lectures ebooks. Je préfère que chaque domaine soit bien respecté. A la fin d'une build, on appelle l'api pour faire le checkup des thumbnails. 
Il faudra que coté backoffice on voit partout ou on peut voir le traitement live des jobs, une phase ou on voit en sse le traitement des thumbnails. Coté api, si on a pas de thumbnail on passe par le code actuel de pages.

---

## 6. Settings API

### Endpoint settings existant
- [ ] Vérifier que `/settings` expose thumbnail config
- [ ] Ajouter endpoint PUT pour mettre à jour thumbnail settings

---

## 7. Taches diverses

- [x] Ajouter dependency `image` et `webp` dans indexer `Cargo.toml`
- [x] Build release vérifié

---

## Ordre d'implémentation suggéré

1. [x] Migration DB + settings
2. [x] Config + parsers (extract first page)
3. [x] Indexer thumbnail generation + save to disk
4. [x] API serve thumbnail
5. [x] Parallel walkdir
6. [ ] Tests & polish (à faire)

---

## Post-déploiement

- [ ] Appliquer migration SQL: `psql -f infra/migrations/0009_add_thumbnails.sql`
- [ ] Créer dossier thumbnails: `mkdir -p /data/thumbnails`
- [ ] Configurer env vars si besoin:
  - `THUMBNAIL_ENABLED=true`
  - `THUMBNAIL_WIDTH=300`
  - `THUMBNAIL_HEIGHT=400`
  - `THUMBNAIL_QUALITY=80`
  - `THUMBNAIL_DIRECTORY=/data/thumbnails`
