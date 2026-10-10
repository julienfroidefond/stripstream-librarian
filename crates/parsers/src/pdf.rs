use std::path::Path;

use anyhow::{Context, Result};

pub fn analyze_pdf(path: &Path, pdf_render_scale: u32) -> Result<(i32, Vec<u8>)> {
    use pdfium_render::prelude::*;

    // Open PDF once — get page count and render first page in a single pass
    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library()
            .map_err(|e| anyhow::anyhow!("pdfium library not available: {:?}", e))?,
    );

    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| anyhow::anyhow!("pdfium load failed for {}: {:?}", path.display(), e))?;

    let count = document.pages().len();
    if count == 0 {
        return Err(anyhow::anyhow!("PDF has no pages: {}", path.display()));
    }

    let scale = if pdf_render_scale == 0 {
        400
    } else {
        pdf_render_scale
    } as i32;
    let config = PdfRenderConfig::new()
        .set_target_width(scale)
        .set_maximum_height(scale);

    let page = document
        .pages()
        .get(0)
        .map_err(|e| anyhow::anyhow!("cannot get first page of {}: {:?}", path.display(), e))?;

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

    Ok((count, buf.into_inner()))
}

pub fn parse_pdf_page_count(path: &Path) -> Result<i32> {
    let doc = lopdf::Document::load(path)
        .with_context(|| format!("cannot open pdf: {}", path.display()))?;
    Ok(doc.get_pages().len() as i32)
}

pub fn render_pdf_page_n(path: &Path, page_number: u32, width: u32) -> Result<Vec<u8>> {
    use pdfium_render::prelude::*;

    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library()
            .map_err(|e| anyhow::anyhow!("pdfium library not available: {:?}", e))?,
    );

    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| anyhow::anyhow!("pdfium load failed for {}: {:?}", path.display(), e))?;

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
