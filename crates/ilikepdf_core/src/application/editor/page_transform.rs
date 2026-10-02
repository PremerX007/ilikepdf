use super::{PageGeometry, PageGeometryError, PageRotation, PdfPoint};

/// Local Flutter/viewport coordinates: x rightward, y downward, in logical units.
/// These values are never persisted as PDF editing coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportRect {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

impl ViewportRect {
    pub fn new(left: f64, top: f64, width: f64, height: f64) -> Result<Self, PageGeometryError> {
        if ![left, top, width, height, left + width, top + height]
            .into_iter()
            .all(f64::is_finite)
            || width <= 0.0
            || height <= 0.0
            || left + width <= left
            || top + height <= top
        {
            return Err(PageGeometryError::InvalidDisplayRectangle);
        }
        Ok(Self {
            left,
            top,
            width,
            height,
        })
    }

    pub fn left(self) -> f64 {
        self.left
    }
    pub fn top(self) -> f64 {
        self.top
    }
    pub fn width(self) -> f64 {
        self.width
    }
    pub fn height(self) -> f64 {
        self.height
    }
}

/// The single PDF ↔ presentation transform for every future editor tool.
/// No DPI, device pixel ratio, PDF engine, or widget dependency is involved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageTransform {
    geometry: PageGeometry,
    display_rect: ViewportRect,
    scale_x: f64,
    scale_y: f64,
}

// Permit only floating-point boundary drift, measured in canonical PDF points.
const BOUNDARY_TOLERANCE_POINTS: f64 = 1e-7;

impl PageTransform {
    /// Use the exact rectangle occupied by the displayed page, excluding padding.
    /// Separate axis scales also accommodate integer raster-dimension rounding.
    pub fn new(
        geometry: PageGeometry,
        display_rect: ViewportRect,
    ) -> Result<Self, PageGeometryError> {
        let scale_x = display_rect.width / geometry.display_width_points();
        let scale_y = display_rect.height / geometry.display_height_points();
        if ![scale_x, scale_y, 1.0 / scale_x, 1.0 / scale_y]
            .into_iter()
            .all(f64::is_finite)
            || scale_x <= 0.0
            || scale_y <= 0.0
        {
            return Err(PageGeometryError::InvalidDisplayRectangle);
        }
        Ok(Self {
            geometry,
            display_rect,
            scale_x,
            scale_y,
        })
    }

    /// Aspect-preserving display; `scale` is logical viewport units per PDF point.
    pub fn at_scale(
        geometry: PageGeometry,
        top_left: ViewportPoint,
        scale: f64,
    ) -> Result<Self, PageGeometryError> {
        Self::new(
            geometry,
            ViewportRect::new(
                top_left.x,
                top_left.y,
                geometry.display_width_points() * scale,
                geometry.display_height_points() * scale,
            )?,
        )
    }

    pub fn geometry(self) -> PageGeometry {
        self.geometry
    }
    pub fn display_rect(self) -> ViewportRect {
        self.display_rect
    }
    pub fn scale_x(self) -> f64 {
        self.scale_x
    }
    pub fn scale_y(self) -> f64 {
        self.scale_y
    }

    /// Closed page boundaries are accepted. Points outside the visible box fail;
    /// no tool-specific clipping or off-page placement policy is applied.
    pub fn pdf_to_viewport(self, point: PdfPoint) -> Result<ViewportPoint, PageGeometryError> {
        finite_point(point.x, point.y)?;
        let visible = self.geometry.visible_box();
        let u = bounded(
            point.x - visible.left_points(),
            self.geometry.width_points(),
            PageGeometryError::PointOutsidePage,
        )?;
        let v = bounded(
            point.y - visible.bottom_points(),
            self.geometry.height_points(),
            PageGeometryError::PointOutsidePage,
        )?;
        let w = self.geometry.width_points();
        let h = self.geometry.height_points();
        let (a, b) = match self.geometry.rotation() {
            PageRotation::None => (u, h - v),
            PageRotation::Clockwise90 => (v, u),
            PageRotation::HalfTurn => (w - u, v),
            PageRotation::Clockwise270 => (h - v, w - u),
        };
        Ok(ViewportPoint {
            x: self.display_rect.left + a * self.scale_x,
            y: self.display_rect.top + b * self.scale_y,
        })
    }

    pub fn viewport_to_pdf(self, point: ViewportPoint) -> Result<PdfPoint, PageGeometryError> {
        finite_point(point.x, point.y)?;
        let a = bounded(
            (point.x - self.display_rect.left) / self.scale_x,
            self.geometry.display_width_points(),
            PageGeometryError::PointOutsideViewport,
        )?;
        let b = bounded(
            (point.y - self.display_rect.top) / self.scale_y,
            self.geometry.display_height_points(),
            PageGeometryError::PointOutsideViewport,
        )?;
        let w = self.geometry.width_points();
        let h = self.geometry.height_points();
        let (u, v) = match self.geometry.rotation() {
            PageRotation::None => (a, h - b),
            PageRotation::Clockwise90 => (b, a),
            PageRotation::HalfTurn => (w - a, b),
            PageRotation::Clockwise270 => (w - b, h - a),
        };
        let visible = self.geometry.visible_box();
        Ok(PdfPoint {
            x: visible.left_points() + u,
            y: visible.bottom_points() + v,
        })
    }
}

fn finite_point(x: f64, y: f64) -> Result<(), PageGeometryError> {
    if x.is_finite() && y.is_finite() {
        Ok(())
    } else {
        Err(PageGeometryError::NonFinitePoint)
    }
}

fn bounded(value: f64, maximum: f64, error: PageGeometryError) -> Result<f64, PageGeometryError> {
    if !value.is_finite()
        || value < -BOUNDARY_TOLERANCE_POINTS
        || value > maximum + BOUNDARY_TOLERANCE_POINTS
    {
        Err(error)
    } else {
        Ok(value.clamp(0.0, maximum))
    }
}

#[cfg(test)]
mod tests;
