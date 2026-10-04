//! In-memory editor foundation. Source PDFs remain read-only; PDF points are canonical.

mod interaction;
mod objects;
mod page_geometry;
mod page_transform;
mod rendering;
mod session;
mod viewport;

pub use interaction::{EditorGesture, EditorGestureKind, EditorResizeHandle};
pub use objects::{
    EDITOR_HISTORY_LIMIT, EDITOR_OBJECT_LIMIT, EditorCommand, EditorEditError, EditorEditState,
    EditorObject, EditorObjectId, EditorObjectKind, EditorRectangle, MIN_OBJECT_SIZE_POINTS,
};
pub use page_geometry::{PageBox, PageGeometry, PageGeometryError, PageRotation, PdfPoint};
pub use page_transform::{PageTransform, ViewportPoint, ViewportRect};
pub use rendering::{EditorPageRaster, render_editor_page};
pub use session::{
    EditorPageMetadata, EditorSession, EditorSessionId, EditorSessionState, open_editor_session,
};
pub use viewport::{
    EditorDocumentLayout, EditorHit, EditorPageLayout, EditorRenderSize, EditorZoom, EditorZoomMode,
};
