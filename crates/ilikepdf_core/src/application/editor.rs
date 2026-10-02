//! Read-only editor foundation. PDF points are canonical; viewport units are presentation-only.

mod page_geometry;
mod page_transform;
mod session;

pub use page_geometry::{PageBox, PageGeometry, PageGeometryError, PageRotation, PdfPoint};
pub use page_transform::{PageTransform, ViewportPoint, ViewportRect};
pub use session::{
    EditorPageMetadata, EditorSession, EditorSessionId, EditorSessionState, open_editor_session,
};
