use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ilikepdf_pdf::{PdfDocumentGeometry, PdfError, PdfPageBox, PdfPageGeometry, PdfPageRotation};

use super::{PageBox, PageGeometry, PageRotation};
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorSessionId(u64);

impl EditorSessionId {
    /// Process-local identity; not a persistent document identifier.
    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorSessionState {
    /// Geometry inspection completed. The session has no edits or native handles.
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorPageMetadata {
    pub page_index: u32,
    pub geometry: PageGeometry,
}

/// An immutable metadata snapshot of one source PDF, in original page order.
/// The source remains read-only. No output, bitmap, password, or native resource
/// is retained. A future workflow reopening the path must revalidate its source.
/// Deliberately has no Debug representation containing a document path.
#[derive(Clone, PartialEq)]
pub struct EditorSession {
    id: EditorSessionId,
    source_path: PathBuf,
    pages: Vec<EditorPageMetadata>,
}

impl EditorSession {
    pub fn id(&self) -> EditorSessionId {
        self.id
    }
    pub fn state(&self) -> EditorSessionState {
        EditorSessionState::ReadOnly
    }
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }
    pub fn page_count(&self) -> u32 {
        self.pages.len() as u32
    }
    pub fn pages(&self) -> &[EditorPageMetadata] {
        &self.pages
    }

    pub fn page(&self, page_index: u32) -> ApplicationResult<&EditorPageMetadata> {
        self.pages.get(page_index as usize).ok_or_else(|| {
            ApplicationError::new(
                ApplicationErrorCode::PageOutOfBounds,
                "The requested editor page does not exist",
            )
        })
    }
}

/// Opens a read-only editor metadata session using the existing bundled PDF runtime.
/// Presentation must use PageTransform for all future PDF ↔ viewport conversion.
pub fn open_editor_session(source_path: &Path) -> ApplicationResult<EditorSession> {
    open_with_inspector(source_path, ilikepdf_pdf::inspect_page_geometry)
}

pub(super) fn open_with_inspector(
    source_path: &Path,
    inspect: impl FnOnce(&Path) -> Result<PdfDocumentGeometry, PdfError>,
) -> ApplicationResult<EditorSession> {
    let document = inspect(source_path).map_err(ApplicationError::from)?;
    if document.pages.is_empty() || u32::try_from(document.pages.len()).is_err() {
        return Err(invalid_geometry());
    }
    let pages = document
        .pages
        .into_iter()
        .enumerate()
        .map(|(index, page)| {
            Ok(EditorPageMetadata {
                page_index: index as u32,
                geometry: convert_geometry(page)?,
            })
        })
        .collect::<ApplicationResult<Vec<_>>>()?;

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map(EditorSessionId)
        .map_err(|_| {
            ApplicationError::new(
                ApplicationErrorCode::Internal,
                "An editor session identity could not be allocated",
            )
        })?;
    Ok(EditorSession {
        id,
        source_path: source_path.to_path_buf(),
        pages,
    })
}

fn convert_geometry(page: PdfPageGeometry) -> ApplicationResult<PageGeometry> {
    let rotation = match page.rotation {
        PdfPageRotation::None => PageRotation::None,
        PdfPageRotation::Clockwise90 => PageRotation::Clockwise90,
        PdfPageRotation::HalfTurn => PageRotation::HalfTurn,
        PdfPageRotation::Clockwise270 => PageRotation::Clockwise270,
    };
    let geometry = PageGeometry::new(
        page.declared_media_box.map(convert_box).transpose()?,
        page.declared_crop_box.map(convert_box).transpose()?,
        convert_box(page.visible_box)?,
        rotation,
    );
    // Native boxes/dimensions have f32 precision. Reject disagreement rather than
    // building a session whose transform silently diverges from rendering.
    for (expected, actual) in [
        (geometry.display_width_points(), page.display_width_points),
        (geometry.display_height_points(), page.display_height_points),
    ] {
        if !actual.is_finite() || (actual - expected).abs() > expected.max(1.0) * 1e-5 {
            return Err(invalid_geometry());
        }
    }
    Ok(geometry)
}

fn convert_box(rect: PdfPageBox) -> ApplicationResult<PageBox> {
    PageBox::new(
        rect.left_points,
        rect.bottom_points,
        rect.right_points,
        rect.top_points,
    )
    .map_err(|_| invalid_geometry())
}

fn invalid_geometry() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::InvalidPdf,
        "The PDF does not have usable editor page geometry",
    )
}

#[cfg(test)]
mod tests;
