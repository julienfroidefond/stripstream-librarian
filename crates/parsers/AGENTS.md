# crates/parsers — Parsing de livres (CBZ, CBR, PDF, EPUB)

Crate utilitaire sans état, utilisée par `apps/api` et `apps/indexer`.
**Aucun binaire externe n'est invoqué** : CBR via le crate `unrar`, PDF via `pdfium-render`.

## API publique (lib.rs)

```rust
// Détection du format — par extension uniquement
pub fn detect_format(path: &Path) -> Option<BookFormat>  // .cbz | .cbr | .pdf | .epub

// Métadonnées depuis le nom de fichier seul — zéro I/O archive, infaillible (phase discovery)
pub fn parse_metadata_fast(path: &Path, format: BookFormat, library_root: &Path) -> ParsedMetadata

// Métadonnées + page_count (ouvre l'archive)
pub fn parse_metadata(path: &Path, format: BookFormat, library_root: &Path) -> Result<ParsedMetadata>

// Ouvre l'archive UNE fois → (page_count, first_page_bytes) — utilisé par l'indexer phase 2
pub fn analyze_book(path: &Path, format: BookFormat, pdf_render_scale: u32) -> Result<(i32, Vec<u8>)>

// Compte les pages sans extraire d'image — chemin de secours quand analyze_book échoue
pub fn page_count_only(path: &Path, format: BookFormat) -> Result<i32>

// PDF bas niveau : comptage, rendu, classification d'erreurs (pdf::PdfError / PdfErrorKind)
// `analyze_pdf` renvoie (page_count, Option<first_page_png>) : le comptage survit à un échec de rendu.
pub mod pdf;  // pdf::analyze_pdf, pdf::pdf_page_count, pdf::render_pdf_page_n, pdf::PdfErrorKind

// Extraction de pages
pub fn extract_first_page(path: &Path, format: BookFormat) -> Result<Vec<u8>>
pub fn extract_page(path: &Path, format: BookFormat, page_number: u32, pdf_render_width: u32) -> Result<Vec<u8>>
pub fn list_archive_images(path: &Path, format: BookFormat) -> Result<Vec<String>>
pub fn extract_image_by_name(path: &Path, format: BookFormat, image_name: &str) -> Result<Vec<u8>>
pub fn is_image_name(name: &str) -> bool

// Conversion CBR → CBZ (écrit le fichier, renvoie son chemin)
pub fn convert_cbr_to_cbz(cbr_path: &Path) -> Result<PathBuf>

pub enum BookFormat { Cbz, Cbr, Pdf, Epub }
pub enum VolumeType { Regular, Hs, Oneshot, Integral }

pub struct ParsedMetadata {
    pub title: String,              // = nom de fichier (sans extension)
    pub series: Option<String>,     // = parent immédiat (ou grandparent si sous-dossier HS)
    pub volume: Option<i32>,        // extrait du nom de fichier
    pub volume_type: VolumeType,    // Regular, Hs, Oneshot, Integral
    pub page_count: Option<i32>,    // None après parse_metadata_fast, rempli par parse_metadata/analyze_book
}
```

`matching.rs` expose la logique de rapprochement titres/releases : `fold_accents`,
`normalize_title`, `title_matches_series`, `title_matches_series_tolerant`, `is_integral_release`,
`match_title_volumes`, `match_release_title`, `match_release_title_tolerant`,
`extract_series_name_from_release`, `extract_series_name_from_filename`
(plus `MatchConfidence::{High,Review}`, `MatchReason`, `ReleaseTitleMatch`).

Les variantes `*_tolerant` absorbent les écarts de nommage des catalogues DDL (connecteurs
`&` ↔ `et`, article initial `Les Légendaires` ↔ `Légendaires`) ; la détection automatique
Prowlarr/Telegram garde la comparaison stricte.

## Logique de parsing

### Titre
Nom de fichier sans extension, conservé tel quel (pas de nettoyage).

### Série
Parent immédiat du fichier. Si le parent matche un pattern HS/special subfolder (`Hors-Série`, `Specials`, `Bonus`, `Extras`, `HS`, `Intégrales`, `INT`), utilise le grandparent :
- `/libraries/One Piece/T01.cbz` → série = `"One Piece"`
- `/libraries/Shonen/Dragon Ball/T01.cbz` → série = `"Dragon Ball"`
- `/libraries/Asterix/Hors-Série/HS1.cbz` → série = `"Asterix"` (HS subfolder skipped)
- `/libraries/one-shot.cbz` → série = `None`

### Volume type (`extract_int_info`, `extract_hs_info`)
Détection dans l'ordre (INT avant HS pour éviter que INTHS matche HS) :
- **Integral** : `INT`, `INTHS`, `Intégrale`, `Integrale` → `VolumeType::Integral`
- **HS** : `HS`, `Hors-Série`, `Spécial`, `Bonus` → `VolumeType::Hs`
- Sinon → `VolumeType::Regular`

### Volume (`extract_volume`)
Patterns reconnus dans le nom de fichier (dans l'ordre de priorité) :
- `T01`, `T1` (manga/comics français)
- `Vol. 1`, `Vol 1`, `Volume 1`
- `#1`, `#01`
- `- 1`, `- 01` (en fin de nom)

Les packs de plage (`T01.T15`, `[T001.T104]`, `T01-T15`, `Tome 01 à Tome 15`) sont expansés.

### Nombre de pages / extraction

| Format | Implémentation |
|--------|----------------|
| CBZ | `zip::ZipArchive` — compte les entrées image (jpg/jpeg/png/webp/avif) |
| CBR | crate `unrar` — listing in-process, filtre les images |
| PDF | `pdfium-render` — comptage + rastérisation via `libpdfium` |
| EPUB | `zip::ZipArchive` — extraction ZIP standard |

`extract_page` dispatche : `extract_cbz_page`, `extract_cbr_page`, `render_pdf_page_n`
(pdfium), `extract_epub_page`.

### PDF : rendu et erreurs

- `pdf::analyze_pdf` renvoie `(page_count, Option<png>)` : le comptage vient de pdfium et est
  **toujours renvoyé** dès que le document s'ouvre, même si le rendu de la première page échoue
  (l'appelant retente la vignette ailleurs). `analyze_book` transforme `None` en erreur.
- Les erreurs pdfium sont classées par `pdf::PdfError` (`PdfErrorKind::{PasswordProtected,
  Corrupted, NoPages, PdfiumUnavailable}`) avec des préfixes stables :
  `pdf::PDF_PASSWORD_ERROR` (« PDF is password-protected ») et `pdf::PDF_CORRUPT_ERROR`.
- Les PDF chiffrés « owner password » uniquement (mot de passe utilisateur vide) s'ouvrent
  normalement ; ceux avec un vrai mot de passe utilisateur ne peuvent pas être lus (aucun mot
  de passe n'est stocké) et sont signalés `PasswordProtected` (`FPDF_ERR_PASSWORD`).

## Dépendances système

**Aucun outil externe.** Les anciens prérequis `unrar`/`unar`/`pdfinfo`/`pdftoppm` ont été supprimés.

- **CBR** : crate `unrar` (in-process, pas de binaire système à installer).
- **PDF** : `libpdfium` doit être disponible sur le chemin des bibliothèques système
  (`Pdfium::bind_to_system_library()`). L'image Docker l'installe depuis `pdfium-binaries`.

Répertoire temp : plus utilisé pour le parsing (les archives sont lues en mémoire).

## Gotchas

- `clean_title()` existe mais est `#[allow(dead_code)]` — le titre n'est **pas** nettoyé (décision volontaire).
- Les CBR peuvent avoir des sous-dossiers internes → listing récursif nécessaire (pas de listing plat).
- La détection du format est **uniquement par extension** (`detect_format`) — pas de magic bytes. Les
  magic bytes ne servent qu'en interne à choisir le bon lecteur (ZIP vs RAR) sur les archives atypiques.
- En cas d'échec de parsing, l'appelant stocke `parse_status = 'error'` sur `book_files` mais continue.
- `parse_metadata_fast` ne touche jamais l'archive — c'est le chemin de la phase discovery de l'indexer.
