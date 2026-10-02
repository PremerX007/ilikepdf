use std::path::Path;

use pdfium_render::prelude::{PdfPageRenderRotation, PdfRect, Pdfium};

use crate::{
    PdfDocumentGeometry, PdfError, PdfErrorKind, PdfPageBox, PdfPageGeometry, PdfPageRotation,
};

pub(crate) fn inspect_page_geometry(
    pdfium: &Pdfium,
    source_path: &Path,
) -> Result<PdfDocumentGeometry, PdfError> {
    super::validate_source(source_path)?;
    let document = pdfium
        .load_pdf_from_file(source_path, None)
        .map_err(super::map_document_load_error)?;
    let mut pages = Vec::with_capacity(document.pages().len() as usize);
    for index in document.pages().as_range() {
        let page = document
            .pages()
            .get(index)
            .map_err(|_| invalid_geometry())?;
        let boundaries = page.boundaries();
        // FPDF_GetPageBoundingBox resolves the effective MediaBox/CropBox,
        // including inherited entries. Do not recompute it from the optional
        // dictionary getters: those do not necessarily include inherited boxes.
        let visible_box = page_box(
            boundaries
                .bounding()
                .map_err(|_| invalid_geometry())?
                .bounds,
        );
        let rotation = match page.rotation().map_err(|_| invalid_geometry())? {
            PdfPageRenderRotation::None => PdfPageRotation::None,
            PdfPageRenderRotation::Degrees90 => PdfPageRotation::Clockwise90,
            PdfPageRenderRotation::Degrees180 => PdfPageRotation::HalfTurn,
            PdfPageRenderRotation::Degrees270 => PdfPageRotation::Clockwise270,
        };
        let display_width_points = f64::from(page.width().value);
        let display_height_points = f64::from(page.height().value);
        if !valid_visible_box(visible_box)
            || !display_width_points.is_finite()
            || !display_height_points.is_finite()
            || display_width_points <= 0.0
            || display_height_points <= 0.0
        {
            return Err(invalid_geometry());
        }
        pages.push(PdfPageGeometry {
            declared_media_box: boundaries
                .media()
                .ok()
                .map(|boundary| page_box(boundary.bounds))
                .filter(|rect| valid_visible_box(*rect)),
            declared_crop_box: boundaries
                .crop()
                .ok()
                .map(|boundary| page_box(boundary.bounds))
                .filter(|rect| valid_visible_box(*rect)),
            visible_box,
            rotation,
            display_width_points,
            display_height_points,
        });
        // The page is dropped on each iteration; no bitmaps or page handles are retained.
    }
    Ok(PdfDocumentGeometry { pages })
}

fn page_box(rect: PdfRect) -> PdfPageBox {
    PdfPageBox {
        left_points: f64::from(rect.left().value),
        bottom_points: f64::from(rect.bottom().value),
        right_points: f64::from(rect.right().value),
        top_points: f64::from(rect.top().value),
    }
}

fn valid_visible_box(rect: PdfPageBox) -> bool {
    [
        rect.left_points,
        rect.bottom_points,
        rect.right_points,
        rect.top_points,
    ]
    .into_iter()
    .all(f64::is_finite)
        && rect.right_points > rect.left_points
        && rect.top_points > rect.bottom_points
}

fn invalid_geometry() -> PdfError {
    PdfError::new(PdfErrorKind::InvalidDocument)
}
