use super::{EditorRenderSize, EditorSession};
use crate::{ApplicationError, ApplicationResult};
use std::io::Cursor;

pub struct EditorPageRaster {
    pub png: Vec<u8>,
    pub size: EditorRenderSize,
}

/// Bounded, in-memory presentation render; never publishes a file or changes the source.
pub fn render_editor_page(
    session: &EditorSession,
    page_index: u32,
    size: EditorRenderSize,
) -> ApplicationResult<EditorPageRaster> {
    session.page(page_index)?;
    let mut output = Cursor::new(Vec::new());
    ilikepdf_pdf::render_page_raster(
        ilikepdf_pdf::PdfPageRasterRequest {
            source_path: session.source_path().to_path_buf(),
            page_index,
            width_pixels: size.width(),
            height_pixels: size.height(),
        },
        &mut output,
    )
    .map_err(ApplicationError::from)?;
    Ok(EditorPageRaster {
        png: output.into_inner(),
        size,
    })
}
