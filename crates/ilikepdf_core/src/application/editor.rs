//! Read-only editor foundation. PDF points are canonical; viewport units are presentation-only.

mod page_geometry;
mod page_transform;
mod rendering;
mod session;
mod viewport;

pub use page_geometry::{PageBox, PageGeometry, PageGeometryError, PageRotation, PdfPoint};
pub use page_transform::{PageTransform, ViewportPoint, ViewportRect};
pub use rendering::{EditorPageRaster, render_editor_page};
pub use session::{
    EditorPageMetadata, EditorSession, EditorSessionId, EditorSessionState, open_editor_session,
};
pub use viewport::{
    EditorDocumentLayout, EditorHit, EditorPageLayout, EditorRenderSize, EditorZoom, EditorZoomMode,
};
