use std::path::Path;

use anyhow::{Context, Result};
use pdfium_render::prelude::Pdfium;

/// Stable error prefixes surfaced to the indexer / API so password-protected
/// documents can be reported distinctly instead of as a generic parse failure.
pub const PDF_PASSWORD_ERROR: &str = "PDF is password-protected";
pub const PDF_CORRUPT_ERROR: &str = "PDF cannot be loaded (corrupted or not a PDF)";

const ENCRYPT_MARKER: &[u8] = b"/Encrypt";
const ENCRYPT_SCAN_WINDOW: u64 = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfErrorKind {
    PasswordProtected,
    Corrupted,
    NoPages,
    PdfiumUnavailable,
}

#[derive(Debug)]
pub struct PdfError {
    pub kind: PdfErrorKind,
    pub detail: String,
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            PdfErrorKind::PasswordProtected => {
                write!(f, "{PDF_PASSWORD_ERROR}: {}", self.detail)
            }
            PdfErrorKind::Corrupted => write!(f, "{PDF_CORRUPT_ERROR}: {}", self.detail),
            PdfErrorKind::NoPages => write!(f, "PDF has no pages: {}", self.detail),
            PdfErrorKind::PdfiumUnavailable => {
                write!(f, "pdfium library not available: {}", self.detail)
            }
        }
    }
}

impl std::error::Error for PdfError {}

impl PdfError {
    fn new(kind: PdfErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn to_anyhow(self) -> anyhow::Error {
        anyhow::Error::new(self)
    }
}

/// True when the given bytes contain an `/Encrypt` marker in the head or tail
/// window (the regions where a PDF trailer lives).
pub fn pdf_bytes_are_encrypted(data: &[u8]) -> bool {
    let head_end = data.len().min(ENCRYPT_SCAN_WINDOW as usize);
    let tail_start = data.len().saturating_sub(ENCRYPT_SCAN_WINDOW as usize);
    let contains_marker = |slice: &[u8]| {
        slice
            .windows(ENCRYPT_MARKER.len())
            .any(|w| w == ENCRYPT_MARKER)
    };
    contains_marker(&data[..head_end]) || contains_marker(&data[tail_start..])
}

/// Reads the head and tail of a file (where the PDF trailer lives).
fn read_file_head_and_tail(path: &Path) -> Vec<u8> {
    use std::io::{Read, Seek, SeekFrom};

    let Ok(mut file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);

    let mut buf = Vec::with_capacity(len.min(ENCRYPT_SCAN_WINDOW * 2) as usize);
    let head_size = len.min(ENCRYPT_SCAN_WINDOW) as usize;
    let mut head = vec![0u8; head_size];
    if file.read_exact(&mut head).is_ok() {
        buf.extend_from_slice(&head);
    }

    if len > ENCRYPT_SCAN_WINDOW
        && file
            .seek(SeekFrom::Start(len - ENCRYPT_SCAN_WINDOW))
            .is_ok()
    {
        let mut tail = Vec::with_capacity(ENCRYPT_SCAN_WINDOW as usize);
        if file.read_to_end(&mut tail).is_ok() {
            buf.extend_from_slice(&tail);
        }
    }

    buf
}

/// True when the file references an `/Encrypt` dictionary, i.e. the document
/// is encrypted (with or without a user password) or is corrupted.
pub fn pdf_file_looks_encrypted(path: &Path) -> bool {
    pdf_bytes_are_encrypted(&read_file_head_and_tail(path))
}

/// Process-global Pdfium instance. Bindings are global in `pdfium-render`, so
/// binding more than once is both wasteful and an error; every caller shares
/// this instance instead.
static PDFIUM: std::sync::OnceLock<Pdfium> = std::sync::OnceLock::new();

fn pdfium() -> Result<&'static Pdfium, PdfError> {
    use pdfium_render::prelude::*;

    if let Some(existing) = PDFIUM.get() {
        return Ok(existing);
    }

    Pdfium::bind_to_system_library()
        .map_err(|e| PdfError::new(PdfErrorKind::PdfiumUnavailable, format!("{e:?}")))
        .map(|bindings| PDFIUM.get_or_init(|| Pdfium::new(bindings)))
}

fn load_document<'a>(
    pdfium: &'a Pdfium,
    path: &Path,
) -> Result<pdfium_render::prelude::PdfDocument<'a>, PdfError> {
    use pdfium_render::prelude::*;

    pdfium.load_pdf_from_file(path, None).map_err(|e| match e {
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
            PdfError::new(PdfErrorKind::PasswordProtected, e.to_string())
        }
        _ if pdf_file_looks_encrypted(path) => {
            PdfError::new(PdfErrorKind::PasswordProtected, e.to_string())
        }
        _ => PdfError::new(PdfErrorKind::Corrupted, format!("{e:?}")),
    })
}

/// Counts the pages of a PDF. Never renders anything: a document whose first
/// page fails to render still reports its page count, so the book stays
/// visible instead of losing its `page_count` entirely.
pub fn pdf_page_count(path: &Path) -> Result<i32> {
    let pdfium = pdfium()?;
    let document = load_document(pdfium, path).map_err(PdfError::to_anyhow)?;
    let count = document.pages().len();
    if count == 0 {
        return Err(PdfError::new(PdfErrorKind::NoPages, path.display().to_string()).to_anyhow());
    }
    Ok(count)
}

/// Opens the PDF once and returns `(page_count, first_page_png)`.
///
/// The page count is authoritative (it comes from pdfium) and is always
/// returned when the document can be opened, even if rendering the first page
/// fails — in that case the rendered bytes are `None` and the caller can retry
/// the thumbnail later. `pdf_render_scale`: max dimension used for the first
/// page rasterization; 0 means default (400).
pub fn analyze_pdf(path: &Path, pdf_render_scale: u32) -> Result<(i32, Option<Vec<u8>>)> {
    use pdfium_render::prelude::*;

    let pdfium = pdfium()?;
    let document = load_document(pdfium, path).map_err(PdfError::to_anyhow)?;

    let count = document.pages().len();
    if count == 0 {
        return Err(PdfError::new(PdfErrorKind::NoPages, path.display().to_string()).to_anyhow());
    }

    let scale = if pdf_render_scale == 0 {
        400
    } else {
        pdf_render_scale
    } as i32;
    let config = PdfRenderConfig::new()
        .set_target_width(scale)
        .set_maximum_height(scale);

    let first_page = match document.pages().get(0) {
        Ok(page) => match page.render_with_config(&config) {
            Ok(bitmap) => match bitmap.as_image() {
                Ok(image) => {
                    let mut buf = std::io::Cursor::new(Vec::new());
                    match image.write_to(&mut buf, image::ImageFormat::Png) {
                        Ok(()) => Some(buf.into_inner()),
                        Err(e) => {
                            tracing::warn!(
                                "PDF first page PNG encoding failed for {}: {}",
                                path.display(),
                                e
                            );
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "PDF first page conversion failed for {}: {:?}",
                        path.display(),
                        e
                    );
                    None
                }
            },
            Err(e) => {
                tracing::warn!(
                    "PDF first page render failed for {} (page count preserved): {:?}",
                    path.display(),
                    e
                );
                None
            }
        },
        Err(e) => {
            tracing::warn!(
                "PDF first page unavailable for {} (page count preserved): {:?}",
                path.display(),
                e
            );
            None
        }
    };

    Ok((count, first_page))
}

/// Legacy page count helper kept for the `parse_metadata` path: pdfium first
/// (same engine as rendering), lopdf as a fallback for documents pdfium
/// refuses to rasterize.
pub fn parse_pdf_page_count(path: &Path) -> Result<i32> {
    if let Ok(count) = pdf_page_count(path) {
        return Ok(count);
    }

    let doc = lopdf::Document::load(path)
        .with_context(|| format!("cannot open pdf: {}", path.display()))?;
    Ok(doc.get_pages().len() as i32)
}

/// Renders page `page_number` (1-based) at the given pixel `width`.
pub fn render_pdf_page_n(path: &Path, page_number: u32, width: u32) -> Result<Vec<u8>> {
    use pdfium_render::prelude::*;

    if page_number == 0 {
        anyhow::bail!("page 0 does not exist in {}", path.display());
    }

    let pdfium = pdfium()?;
    let document = load_document(pdfium, path).map_err(PdfError::to_anyhow)?;

    let page_index = (page_number - 1) as i32;
    let page = document
        .pages()
        .get(page_index)
        .map_err(|_| anyhow::anyhow!("page {} out of range in {}", page_number, path.display()))?;

    let config = PdfRenderConfig::new().set_target_width(width as i32);

    let bitmap = page
        .render_with_config(&config)
        .map_err(|e| anyhow::anyhow!("pdfium render failed for {}: {:?}", path.display(), e))?;

    let image = bitmap
        .as_image()
        .map_err(|e| anyhow::anyhow!("pdfium image conversion failed: {:?}", e))?;
    let mut buf = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut buf, image::ImageFormat::Png)
        .context("failed to encode rendered PDF page as PNG")?;

    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test fixtures generated with reportlab + pypdf (see the PR description).
    // `owner_only`: AES-128, empty user password, non-empty owner password
    // (opens without a password, permissions restricted).
    // `user_password`: AES-128, a real user password → pdfium cannot open it.
    const PLAIN_PDF_B64: &str = "JVBERi0xLjMKJZOMi54gUmVwb3J0TGFiIEdlbmVyYXRlZCBQREYgZG9jdW1lbnQgKG9wZW5zb3VyY2UpCjEgMCBvYmoKPDwKL0YxIDIgMCBSCj4+CmVuZG9iagoyIDAgb2JqCjw8Ci9CYXNlRm9udCAvSGVsdmV0aWNhIC9FbmNvZGluZyAvV2luQW5zaUVuY29kaW5nIC9OYW1lIC9GMSAvU3VidHlwZSAvVHlwZTEgL1R5cGUgL0ZvbnQKPj4KZW5kb2JqCjMgMCBvYmoKPDwKL0NvbnRlbnRzIDggMCBSIC9NZWRpYUJveCBbIDAgMCA1OTUuMjc1NiA4NDEuODg5OCBdIC9QYXJlbnQgNyAwIFIgL1Jlc291cmNlcyA8PAovRm9udCAxIDAgUiAvUHJvY1NldCBbIC9QREYgL1RleHQgL0ltYWdlQiAvSW1hZ2VDIC9JbWFnZUkgXQo+PiAvUm90YXRlIDAgL1RyYW5zIDw8Cgo+PiAKICAvVHlwZSAvUGFnZQo+PgplbmRvYmoKNCAwIG9iago8PAovQ29udGVudHMgOSAwIFIgL01lZGlhQm94IFsgMCAwIDU5NS4yNzU2IDg0MS44ODk4IF0gL1BhcmVudCA3IDAgUiAvUmVzb3VyY2VzIDw8Ci9Gb250IDEgMCBSIC9Qcm9jU2V0IFsgL1BERiAvVGV4dCAvSW1hZ2VCIC9JbWFnZUMgL0ltYWdlSSBdCj4+IC9Sb3RhdGUgMCAvVHJhbnMgPDwKCj4+IAogIC9UeXBlIC9QYWdlCj4+CmVuZG9iago1IDAgb2JqCjw8Ci9QYWdlTW9kZSAvVXNlTm9uZSAvUGFnZXMgNyAwIFIgL1R5cGUgL0NhdGFsb2cKPj4KZW5kb2JqCjYgMCBvYmoKPDwKL0F1dGhvciAoYW5vbnltb3VzKSAvQ3JlYXRpb25EYXRlIChEOjIwMjYxMDEzMTMwOTQ2KzAyJzAwJykgL0NyZWF0b3IgKGFub255bW91cykgL0tleXdvcmRzICgpIC9Nb2REYXRlIChEOjIwMjYxMDEzMTMwOTQ2KzAyJzAwJykgL1Byb2R1Y2VyIChSZXBvcnRMYWIgUERGIExpYnJhcnkgLSBcKG9wZW5zb3VyY2VcKSkgCiAgL1N1YmplY3QgKHVuc3BlY2lmaWVkKSAvVGl0bGUgKHVudGl0bGVkKSAvVHJhcHBlZCAvRmFsc2UKPj4KZW5kb2JqCjcgMCBvYmoKPDwKL0NvdW50IDIgL0tpZHMgWyAzIDAgUiA0IDAgUiBdIC9UeXBlIC9QYWdlcwo+PgplbmRvYmoKOCAwIG9iago8PAovRmlsdGVyIFsgL0FTQ0lJODVEZWNvZGUgL0ZsYXRlRGVjb2RlIF0gL0xlbmd0aCA5NQo+PgpzdHJlYW0KR2FwUWgwRT1GLDBVXEgzVFxwTllUXlFLaz90Yz5JUCw7VyNVMV4yM2loUEVNXz9DVzRLSVNpPCFbN2AjT0Jfc0tLI0k6XCJbYyI8Tz1aKHMvV19eay9jX1BaJkQ3fj5lbmRzdHJlYW0KZW5kb2JqCjkgMCBvYmoKPDwKL0ZpbHRlciBbIC9BU0NJSTg1RGVjb2RlIC9GbGF0ZURlY29kZSBdIC9MZW5ndGggOTUKPj4Kc3RyZWFtCkdhcFFoMEU9RiwwVVxIM1RccE5ZVF5RS2s/dGM+SVAsO1cjVTFeMjNpaFBFTV8/Q1c0S0lTaTwhWzdgI09CX3NLSyNJOlwiWD9gcU89WihzL1dfXmsvY19QaCZEQH4+ZW5kc3RyZWFtCmVuZG9iagp4cmVmCjAgMTAKMDAwMDAwMDAwMCA2NTUzNSBmIAowMDAwMDAwMDYxIDAwMDAwIG4gCjAwMDAwMDAwOTIgMDAwMDAgbiAKMDAwMDAwMDE5OSAwMDAwMCBuIAowMDAwMDAwNDAyIDAwMDAwIG4gCjAwMDAwMDA2MDUgMDAwMDAgbiAKMDAwMDAwMDY3MyAwMDAwMCBuIAowMDAwMDAwOTM0IDAwMDAwIG4gCjAwMDAwMDA5OTkgMDAwMDAgbiAKMDAwMDAwMTE4MyAwMDAwMCBuIAp0cmFpbGVyCjw8Ci9JRCAKWzxjNjZkNDNjODc3NDZjYjE4MjY1YmE3ZTlhMTUxNDA1NT48YzY2ZDQzYzg3NzQ2Y2IxODI2NWJhN2U5YTE1MTQwNTU+XQolIFJlcG9ydExhYiBnZW5lcmF0ZWQgUERGIGRvY3VtZW50IC0tIGRpZ2VzdCAob3BlbnNvdXJjZSkKCi9JbmZvIDYgMCBSCi9Sb290IDUgMCBSCi9TaXplIDEwCj4+CnN0YXJ0eHJlZgoxMzY3CiUlRU9GCg==";
    const OWNER_ONLY_PDF_B64: &str = "JVBERi0xLjMKJeLjz9MKMSAwIG9iago8PAovUHJvZHVjZXIgPDAzZGYyM2ViYWY+Cj4+CmVuZG9iagoyIDAgb2JqCjw8Ci9UeXBlIC9QYWdlcwovQ291bnQgMgovS2lkcyBbIDQgMCBSIDggMCBSIF0KPj4KZW5kb2JqCjMgMCBvYmoKPDwKL1R5cGUgL0NhdGFsb2cKL1BhZ2VzIDIgMCBSCj4+CmVuZG9iago0IDAgb2JqCjw8Ci9Db250ZW50cyA1IDAgUgovTWVkaWFCb3ggWyAwIDAgNTk1LjI3NTYgODQxLjg4OTggXQovUmVzb3VyY2VzIDw8Ci9Gb250IDYgMCBSCi9Qcm9jU2V0IFsgL1BERiAvVGV4dCAvSW1hZ2VCIC9JbWFnZUMgL0ltYWdlSSBdCj4+Ci9Sb3RhdGUgMAovVHJhbnMgPDwKPj4KL1R5cGUgL1BhZ2UKL1BhcmVudCAyIDAgUgo+PgplbmRvYmoKNSAwIG9iago8PAovRmlsdGVyIFsgL0FTQ0lJODVEZWNvZGUgL0ZsYXRlRGVjb2RlIF0KL0xlbmd0aCA5NQo+PgpzdHJlYW0KJHBNAgSxEqokq4Lp9vtXuWBLaBNYgz8B4pUWWOJmtD7yNbN16JlTcxhF62tjVgg55iV+mn8U7nOvkGhOXVrJ1nKRLRQ4Hqe900ngmdDIjXPVGH9rXVjbYTeJjT2inrIKZW5kc3RyZWFtCmVuZG9iago2IDAgb2JqCjw8Ci9GMSA3IDAgUgo+PgplbmRvYmoKNyAwIG9iago8PAovQmFzZUZvbnQgL0hlbHZldGljYQovRW5jb2RpbmcgL1dpbkFuc2lFbmNvZGluZwovTmFtZSAvRjEKL1N1YnR5cGUgL1R5cGUxCi9UeXBlIC9Gb250Cj4+CmVuZG9iago4IDAgb2JqCjw8Ci9Db250ZW50cyA5IDAgUgovTWVkaWFCb3ggWyAwIDAgNTk1LjI3NTYgODQxLjg4OTggXQovUmVzb3VyY2VzIDw8Ci9Gb250IDYgMCBSCi9Qcm9jU2V0IFsgL1BERiAvVGV4dCAvSW1hZ2VCIC9JbWFnZUMgL0ltYWdlSSBdCj4+Ci9Sb3RhdGUgMAovVHJhbnMgPDwKPj4KL1R5cGUgL1BhZ2UKL1BhcmVudCAyIDAgUgo+PgplbmRvYmoKOSAwIG9iago8PAovRmlsdGVyIFsgL0FTQ0lJODVEZWNvZGUgL0ZsYXRlRGVjb2RlIF0KL0xlbmd0aCA5NQo+PgpzdHJlYW0KiEXhuUBGeZl4Ff8CXx9uxuwE321e2Hj/ITbKRVDkRF3seGfNPDqy6C93YEl7R5ckvpxmAAhvsziIU5dRX/svWpBdywPlTaCDQpPXla3DqMGnmxlf3UEYRBQtywQ1NPwKZW5kc3RyZWFtCmVuZG9iagoxMCAwIG9iago8PAovViAyCi9SIDMKL0xlbmd0aCAxMjgKL1AgNDI5NDk2NzI5MgovRmlsdGVyIC9TdGFuZGFyZAovTyA8OTEzYjA3M2Y1YjY1OTU0YzhiNmRhMjIxNjk5NzFhMTc3YjdlNmQ5NGExMDZmZjc1YzBjMTYyOTZiYzE1NDEyZT4KL1UgPDMwNTZkYTBiOTVjN2RlY2UyMmI2Y2I1ZGFlMGUyMTUxMjhiZjRlNWU0ZTc1OGE0MTY0MDA0ZTU2ZmZmYTAxMDg+Cj4+CmVuZG9iagp4cmVmCjAgMTEKMDAwMDAwMDAwMCA2NTUzNSBmIAowMDAwMDAwMDE1IDAwMDAwIG4gCjAwMDAwMDAwNTkgMDAwMDAgbiAKMDAwMDAwMDEyNCAwMDAwMCBuIAowMDAwMDAwMTczIDAwMDAwIG4gCjAwMDAwMDAzNzIgMDAwMDAgbiAKMDAwMDAwMDU1NyAwMDAwMCBuIAowMDAwMDAwNTg4IDAwMDAwIG4gCjAwMDAwMDA2OTUgMDAwMDAgbiAKMDAwMDAwMDg5NCAwMDAwMCBuIAowMDAwMDAxMDc5IDAwMDAwIG4gCnRyYWlsZXIKPDwKL1NpemUgMTEKL1Jvb3QgMyAwIFIKL0luZm8gMSAwIFIKL0lEIFsgPDM1MzY2NDY1Mzk2MjY2MzAzMzM3NjQ2NDM1MzMzMjM2NjMzNjM5MzkzNjM3MzUzNjM0MzczNzMyMzgzMTMxMzg+IDwzNTM2NjQ2NTM5NjI2NjMwMzMzNzY0NjQzNTMzMzIzNjYzMzYzOTM5MzYzNzM1MzYzNDM3MzczMjM4MzEzMTM4PiBdCi9FbmNyeXB0IDEwIDAgUgo+PgpzdGFydHhyZWYKMTI5NQolJUVPRgo=";
    const USER_PASSWORD_PDF_B64: &str = "JVBERi0xLjMKJeLjz9MKMSAwIG9iago8PAovUHJvZHVjZXIgPDFiNzE4ZDQ2NGU+Cj4+CmVuZG9iagoyIDAgb2JqCjw8Ci9UeXBlIC9QYWdlcwovQ291bnQgMgovS2lkcyBbIDQgMCBSIDggMCBSIF0KPj4KZW5kb2JqCjMgMCBvYmoKPDwKL1R5cGUgL0NhdGFsb2cKL1BhZ2VzIDIgMCBSCj4+CmVuZG9iago0IDAgb2JqCjw8Ci9Db250ZW50cyA1IDAgUgovTWVkaWFCb3ggWyAwIDAgNTk1LjI3NTYgODQxLjg4OTggXQovUmVzb3VyY2VzIDw8Ci9Gb250IDYgMCBSCi9Qcm9jU2V0IFsgL1BERiAvVGV4dCAvSW1hZ2VCIC9JbWFnZUMgL0ltYWdlSSBdCj4+Ci9Sb3RhdGUgMAovVHJhbnMgPDwKPj4KL1R5cGUgL1BhZ2UKL1BhcmVudCAyIDAgUgo+PgplbmRvYmoKNSAwIG9iago8PAovRmlsdGVyIFsgL0FTQ0lJODVEZWNvZGUgL0ZsYXRlRGVjb2RlIF0KL0xlbmd0aCA5NQo+PgpzdHJlYW0KlmErW9sFhJjEn+POhsErRtrT3Kb/1N0CnGh3V8NQQL/hyErzJ0x28EOcPCer4rPodBSTCnfnrSFZTBENIwDkX9e4wqnzYJSaJI9wNEglJR/cUjlbnL4zH9EiaGkRes0KZW5kc3RyZWFtCmVuZG9iago2IDAgb2JqCjw8Ci9GMSA3IDAgUgo+PgplbmRvYmoKNyAwIG9iago8PAovQmFzZUZvbnQgL0hlbHZldGljYQovRW5jb2RpbmcgL1dpbkFuc2lFbmNvZGluZwovTmFtZSAvRjEKL1N1YnR5cGUgL1R5cGUxCi9UeXBlIC9Gb250Cj4+CmVuZG9iago4IDAgb2JqCjw8Ci9Db250ZW50cyA5IDAgUgovTWVkaWFCb3ggWyAwIDAgNTk1LjI3NTYgODQxLjg4OTggXQovUmVzb3VyY2VzIDw8Ci9Gb250IDYgMCBSCi9Qcm9jU2V0IFsgL1BERiAvVGV4dCAvSW1hZ2VCIC9JbWFnZUMgL0ltYWdlSSBdCj4+Ci9Sb3RhdGUgMAovVHJhbnMgPDwKPj4KL1R5cGUgL1BhZ2UKL1BhcmVudCAyIDAgUgo+PgplbmRvYmoKOSAwIG9iago8PAovRmlsdGVyIFsgL0FTQ0lJODVEZWNvZGUgL0ZsYXRlRGVjb2RlIF0KL0xlbmd0aCA5NQo+PgpzdHJlYW0Ko4jqf1PbUg1N2SQFi0TL2UJMAUoyrK18y4q8C/m5oXljJdd4Yx6GRl6s98Vz/WQ0qUzoED7UOKq9dm6O7+DmFt+QjJBL+LkF+B4wW4wI8zplvZ5BOMGBuXOl8jMkOpcKZW5kc3RyZWFtCmVuZG9iagoxMCAwIG9iago8PAovViAyCi9SIDMKL0xlbmd0aCAxMjgKL1AgNDI5NDk2NzI5MgovRmlsdGVyIC9TdGFuZGFyZAovTyA8Y2NmNzJjMTMzODYzN2E2ZTlkMDg5ODVmMjkyMzQ1NTEyMGRhMmM0NjcxMjA5NzBhMTVjY2MzNDZmNjQ2OWU4ND4KL1UgPGM5ZGI5MWZhMmMwMDdmYzNhZDkxNWVlYzA4OWEyYjBkMjhiZjRlNWU0ZTc1OGE0MTY0MDA0ZTU2ZmZmYTAxMDg+Cj4+CmVuZG9iagp4cmVmCjAgMTEKMDAwMDAwMDAwMCA2NTUzNSBmIAowMDAwMDAwMDE1IDAwMDAwIG4gCjAwMDAwMDAwNTkgMDAwMDAgbiAKMDAwMDAwMDEyNCAwMDAwMCBuIAowMDAwMDAwMTczIDAwMDAwIG4gCjAwMDAwMDAzNzIgMDAwMDAgbiAKMDAwMDAwMDU1NyAwMDAwMCBuIAowMDAwMDAwNTg4IDAwMDAwIG4gCjAwMDAwMDA2OTUgMDAwMDAgbiAKMDAwMDAwMDg5NCAwMDAwMCBuIAowMDAwMDAxMDc5IDAwMDAwIG4gCnRyYWlsZXIKPDwKL1NpemUgMTEKL1Jvb3QgMyAwIFIKL0luZm8gMSAwIFIKL0lEIFsgPDM1MzY2NDY1Mzk2MjY2MzAzMzM3NjQ2NDM1MzMzMjM2NjMzNjM5MzkzNjM3MzUzNjM0MzczNzMyMzgzMTMxMzg+IDwzNTM2NjQ2NTM5NjI2NjMwMzMzNzY0NjQzNTMzMzIzNjYzMzYzOTM5MzYzNzM1MzYzNDM3MzczMjM4MzEzMTM4PiBdCi9FbmNyeXB0IDEwIDAgUgo+PgpzdGFydHhyZWYKMTI5NQolJUVPRgo=";

    fn decode_base64(input: &str) -> Vec<u8> {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut lookup = [255u8; 256];
        for (i, &c) in TABLE.iter().enumerate() {
            lookup[c as usize] = i as u8;
        }
        let mut out = Vec::with_capacity(input.len() / 4 * 3);
        let mut buffer = 0u32;
        let mut bits = 0u32;
        for &byte in input.as_bytes() {
            let value = lookup[byte as usize];
            if value == 255 {
                continue;
            }
            buffer = (buffer << 6) | value as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buffer >> bits) as u8);
            }
        }
        out
    }

    fn pdfium_available() -> bool {
        use pdfium_render::prelude::*;
        Pdfium::bind_to_system_library().is_ok()
    }

    fn write_fixture(dir: &std::path::Path, name: &str, b64: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, decode_base64(b64)).expect("write fixture");
        path
    }

    #[test]
    fn base64_decoder_roundtrip() {
        // "PDF" -> "UERG"
        assert_eq!(decode_base64("UERG"), b"PDF");
        assert_eq!(decode_base64("U"), Vec::<u8>::new());
    }

    #[test]
    fn encrypted_marker_detection() {
        let encrypted = decode_base64(OWNER_ONLY_PDF_B64);
        assert!(pdf_bytes_are_encrypted(&encrypted));
        let plain = decode_base64(PLAIN_PDF_B64);
        assert!(!pdf_bytes_are_encrypted(&plain));
        // Marker in the head only.
        let mut head_marker = b"%PDF-1.4\n".to_vec();
        head_marker.extend_from_slice(b"/Encrypt 5 0 R\n");
        head_marker.resize(64, 0);
        assert!(pdf_bytes_are_encrypted(&head_marker));
        assert!(!pdf_bytes_are_encrypted(b"no marker here"));
    }

    #[test]
    fn page_count_ignores_encryption_password() {
        let dir = tempfile::tempdir().unwrap();
        let plain = write_fixture(dir.path(), "plain.pdf", PLAIN_PDF_B64);
        let owner_only = write_fixture(dir.path(), "owner_only.pdf", OWNER_ONLY_PDF_B64);

        if !pdfium_available() {
            eprintln!("skipping: libpdfium not available");
            return;
        }

        assert_eq!(pdf_page_count(&plain).unwrap(), 2);
        // Owner-password-only PDFs open with an empty user password.
        assert_eq!(pdf_page_count(&owner_only).unwrap(), 2);
        // A file that is not a PDF at all is reported as corrupted.
        let not_a_pdf = dir.path().join("fake.pdf");
        std::fs::write(&not_a_pdf, b"this is not a pdf").unwrap();
        let error = pdf_page_count(&not_a_pdf).unwrap_err();
        let kind = error
            .downcast_ref::<PdfError>()
            .map(|e| e.kind)
            .expect("typed PdfError");
        assert_eq!(kind, PdfErrorKind::Corrupted);
    }

    #[test]
    fn analyze_pdf_returns_page_count_and_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let plain = write_fixture(dir.path(), "plain.pdf", PLAIN_PDF_B64);

        if !pdfium_available() {
            eprintln!("skipping: libpdfium not available");
            return;
        }

        let (page_count, first_page) = analyze_pdf(&plain, 400).unwrap();
        assert_eq!(page_count, 2);
        let bytes = first_page.expect("first page rendered");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
    }

    #[test]
    fn analyze_pdf_classifies_password_protected_documents() {
        let dir = tempfile::tempdir().unwrap();
        let user_pw = write_fixture(dir.path(), "user_pw.pdf", USER_PASSWORD_PDF_B64);

        if !pdfium_available() {
            eprintln!("skipping: libpdfium not available");
            return;
        }

        let error = analyze_pdf(&user_pw, 400).unwrap_err();
        let pdf_error = error.downcast_ref::<PdfError>().expect("typed PdfError");
        assert_eq!(pdf_error.kind, PdfErrorKind::PasswordProtected);
        assert!(pdf_error.to_string().contains(PDF_PASSWORD_ERROR));
    }

    #[test]
    fn owner_only_encrypted_pdf_can_be_analyzed() {
        let dir = tempfile::tempdir().unwrap();
        let owner_only = write_fixture(dir.path(), "owner_only.pdf", OWNER_ONLY_PDF_B64);

        if !pdfium_available() {
            eprintln!("skipping: libpdfium not available");
            return;
        }

        // Encrypted for permissions but viewable: must load, count and render.
        let (page_count, first_page) = analyze_pdf(&owner_only, 400).unwrap();
        assert_eq!(page_count, 2);
        assert!(first_page.is_some());
    }

    #[test]
    fn analyze_book_pdf_without_renderable_page_still_counts() {
        use crate::BookFormat;

        let dir = tempfile::tempdir().unwrap();
        let user_pw = write_fixture(dir.path(), "user_pw.pdf", USER_PASSWORD_PDF_B64);

        if !pdfium_available() {
            eprintln!("skipping: libpdfium not available");
            return;
        }

        // Fully unreadable PDF: both paths report the password error, and the
        // error text stays actionable for the UI.
        let analyze_error = crate::analyze_book(&user_pw, BookFormat::Pdf, 400).unwrap_err();
        assert!(analyze_error.to_string().contains(PDF_PASSWORD_ERROR));
        let count_error = crate::page_count_only(&user_pw, BookFormat::Pdf).unwrap_err();
        assert!(count_error.to_string().contains(PDF_PASSWORD_ERROR));
    }
}
