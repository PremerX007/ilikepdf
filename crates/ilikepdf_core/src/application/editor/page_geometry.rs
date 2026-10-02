use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Unrotated source-page coordinates: x rightward, y upward, in PDF points.
/// (0, 0) is the PDF user-space origin, not necessarily a visible-page corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRotation {
    None,
    Clockwise90,
    HalfTurn,
    Clockwise270,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageGeometryError {
    InvalidPageBox,
    InvalidDisplayRectangle,
    NonFinitePoint,
    PointOutsidePage,
    PointOutsideViewport,
}

impl Display for PageGeometryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPageBox => "Page box must have finite positive dimensions",
            Self::InvalidDisplayRectangle => {
                "Display rectangle must have finite positive dimensions"
            }
            Self::NonFinitePoint => "Point coordinates must be finite",
            Self::PointOutsidePage => "Point is outside the visible page box",
            Self::PointOutsideViewport => "Point is outside the displayed page rectangle",
        })
    }
}

impl Error for PageGeometryError {}

/// A validated, unrotated box in source PDF points. Negative origins are valid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBox {
    left: f64,
    bottom: f64,
    right: f64,
    top: f64,
}

impl PageBox {
    pub fn new(left: f64, bottom: f64, right: f64, top: f64) -> Result<Self, PageGeometryError> {
        if ![left, bottom, right, top, right - left, top - bottom]
            .into_iter()
            .all(f64::is_finite)
            || right <= left
            || top <= bottom
        {
            return Err(PageGeometryError::InvalidPageBox);
        }
        Ok(Self {
            left,
            bottom,
            right,
            top,
        })
    }

    pub fn left_points(self) -> f64 {
        self.left
    }
    pub fn bottom_points(self) -> f64 {
        self.bottom
    }
    pub fn right_points(self) -> f64 {
        self.right
    }
    pub fn top_points(self) -> f64 {
        self.top
    }
    pub fn width_points(self) -> f64 {
        self.right - self.left
    }
    pub fn height_points(self) -> f64 {
        self.top - self.bottom
    }
}

/// Immutable engine-neutral geometry. Visible dimensions derive from the resolved
/// box, never from the optional declared boxes or painted content bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGeometry {
    declared_media_box: Option<PageBox>,
    declared_crop_box: Option<PageBox>,
    visible_box: PageBox,
    rotation: PageRotation,
}

impl PageGeometry {
    /// `visible_box` must be the inspector's resolved effective box, including
    /// inheritance/defaults and MediaBox/CropBox intersection. Declared boxes
    /// are optional provenance only and never determine the transform.
    pub fn new(
        declared_media_box: Option<PageBox>,
        declared_crop_box: Option<PageBox>,
        visible_box: PageBox,
        rotation: PageRotation,
    ) -> Self {
        Self {
            declared_media_box,
            declared_crop_box,
            visible_box,
            rotation,
        }
    }

    pub fn declared_media_box(self) -> Option<PageBox> {
        self.declared_media_box
    }
    pub fn declared_crop_box(self) -> Option<PageBox> {
        self.declared_crop_box
    }
    pub fn visible_box(self) -> PageBox {
        self.visible_box
    }
    pub fn rotation(self) -> PageRotation {
        self.rotation
    }
    /// Visible width before intrinsic rotation, in PDF points.
    pub fn width_points(self) -> f64 {
        self.visible_box.width_points()
    }
    /// Visible height before intrinsic rotation, in PDF points.
    pub fn height_points(self) -> f64 {
        self.visible_box.height_points()
    }

    pub fn display_width_points(self) -> f64 {
        match self.rotation {
            PageRotation::None | PageRotation::HalfTurn => self.width_points(),
            PageRotation::Clockwise90 | PageRotation::Clockwise270 => self.height_points(),
        }
    }

    pub fn display_height_points(self) -> f64 {
        match self.rotation {
            PageRotation::None | PageRotation::HalfTurn => self.height_points(),
            PageRotation::Clockwise90 | PageRotation::Clockwise270 => self.width_points(),
        }
    }
}

#[cfg(test)]
mod tests;
