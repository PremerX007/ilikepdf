//! Engine-neutral, read-only page inspection results. No native handles escape.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfPageBox {
    pub left_points: f64,
    pub bottom_points: f64,
    pub right_points: f64,
    pub top_points: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfPageRotation {
    None,
    Clockwise90,
    HalfTurn,
    Clockwise270,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfPageGeometry {
    /// Normalized, non-degenerate page-dictionary box; inherited values may be absent.
    pub declared_media_box: Option<PdfPageBox>,
    /// Normalized, non-degenerate page-dictionary box; inherited values may be absent.
    pub declared_crop_box: Option<PdfPageBox>,
    /// Resolved visible box, including inheritance, defaults, and box intersection.
    /// This is a page boundary, not the bounding box of painted content.
    pub visible_box: PdfPageBox,
    pub rotation: PdfPageRotation,
    /// Native visible dimensions after intrinsic rotation, before display scaling.
    pub display_width_points: f64,
    pub display_height_points: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfDocumentGeometry {
    /// Source order; the vector index is the zero-based source page index.
    pub pages: Vec<PdfPageGeometry>,
}
