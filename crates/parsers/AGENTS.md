# crates/parsers — Parsing de livres (CBZ, CBR, PDF)

Crate utilitaire sans état, utilisée par `apps/api` et `apps/indexer`.

## API publique (lib.rs)

```rust
// Détection du format par extension
pub fn detect_format(path: &Path) -> Option<BookFormat>  // .cbz | .cbr | .pdf

// Extraction des métadonnées
pub fn parse_metadata(path: &Path, format: BookFormat, library_root: &Path) -> Result<ParsedMetadata>

// Extraction de la première page (pour thumbnails)
pub fn extract_first_page(path: &Path, format: BookFormat) -> Result<Vec<u8>>

pub enum BookFormat { Cbz, Cbr, Pdf }

pub struct ParsedMetadata {
    pub title: String,              // = nom de fichier (sans extension)
    pub series: Option<String>,     // = parent immédiat (ou grandparent si sous-dossier HS)
    pub volume: Option<i32>,        // extrait du nom de fichier
    pub volume_type: VolumeType,    // Regular, Hs, Oneshot, Integral
    pub page_count: Option<i32>,
}

pub enum VolumeType { Regular, Hs, Oneshot, Integral }
```

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

### Nombre de pages
| Format | Outil |
|--------|-------|
| CBZ | `zip::ZipArchive` — compte les entrées image (jpg/jpeg/png/webp/avif) |
| CBR | `unrar lb <path>` — liste les fichiers, filtre les images |
| PDF | `pdfinfo <path>` — lit la ligne `Pages:` |

## Dépendances système requises

| Outil | Utilisé pour | Installation |
|-------|-------------|-------------|
| `unrar` | CBR page count | `brew install rar` / `apt install unrar` |
| `unar` | CBR first page extraction | `brew install unar` / `apt install unar` |
| `pdfinfo` | PDF page count | inclus dans `poppler-utils` |
| `pdftoppm` | PDF first page render | inclus dans `poppler-utils` |

**Important** : `unrar` (pour le listing) et `unar` (pour l'extraction) sont deux outils différents.

## Extraction première page

- **CBZ** : `zip::ZipArchive`, trie les noms d'images, lit la première
- **CBR** : `unar -o <tmp_dir>`, `WalkDir` récursif, trie, lit la première — nettoie `tmp_dir` ensuite
- **PDF** : `pdftoppm -f 1 -singlefile -png -scale-to 800` → fichier PNG temporaire — nettoie `tmp_dir` ensuite

Répertoire temp : `std::env::temp_dir()/stripstream-{cbr|pdf}-thumb-<uuid>`.

## Gotchas

- `clean_title()` existe mais est marqué `#[allow(dead_code)]` — le titre n'est **pas** nettoyé (décision volontaire).
- Les CBR peuvent avoir des sous-dossiers internes → WalkDir nécessaire (pas de listing plat).
- La détection du format est **uniquement par extension** (pas de magic bytes).
- `pdfinfo` et `pdftoppm` doivent être du paquet `poppler-utils` (pas `poppler` seul).
- En cas d'échec de parsing, l'appelant (indexer/api) stocke `parse_status = 'error'` en DB mais continue.
