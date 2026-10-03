use super::{
    EditorPageMetadata, EditorSession, PageGeometry, PageGeometryError, PageTransform, PdfPoint,
    ViewportPoint, ViewportRect,
};

/// Presentation units per source PDF point. 100% is 1, independent of physical DPI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorZoom(f64);

impl EditorZoom {
    pub const MIN: f64 = 0.25;
    pub const MAX: f64 = 4.0;
    pub fn new(scale: f64) -> Result<Self, PageGeometryError> {
        if !scale.is_finite() || !(Self::MIN..=Self::MAX).contains(&scale) {
            return Err(PageGeometryError::InvalidDisplayRectangle);
        }
        Ok(Self(scale))
    }
    pub fn scale(self) -> f64 {
        self.0
    }
    pub fn stepped(self, increase: bool) -> Self {
        Self((self.0 * if increase { 1.25 } else { 0.8 }).clamp(Self::MIN, Self::MAX))
    }
}

impl Default for EditorZoom {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditorZoomMode {
    Custom(EditorZoom),
    FitWidth,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorPageLayout {
    pub page_index: u32,
    pub geometry: PageGeometry,
    pub transform: PageTransform,
    pub zoom: EditorZoom,
}

impl EditorPageLayout {
    /// One authoritative document-surface rectangle, shared by raster and overlays.
    pub fn rect(self) -> ViewportRect {
        self.transform.display_rect()
    }
    /// Density affects raster quality only; it never changes page geometry or zoom.
    pub fn render_size(self, density: f64) -> Result<EditorRenderSize, PageGeometryError> {
        if !density.is_finite() || density <= 0.0 || density > 8.0 {
            return Err(PageGeometryError::InvalidDisplayRectangle);
        }
        let width = self.rect().width() * density;
        let height = self.rect().height() * density;
        // Round physical resolution up in small, aspect-preserving buckets so
        // tiny fit-width changes reuse a raster without undersampling text.
        let longest = width.max(height);
        let bucket = (longest / 64.0).ceil() * 64.0;
        // Derive aspect from source geometry, independent of zoom's floating
        // point rounding, so the same bucket always produces the same key.
        let ratio = self.geometry.display_width_points() / self.geometry.display_height_points();
        let unit_width = ratio.min(1.0);
        let unit_height = (1.0 / ratio).min(1.0);
        let target = bucket
            .min(f64::from(EditorRenderSize::MAX_AXIS))
            .min((EditorRenderSize::MAX_PIXELS as f64 / (unit_width * unit_height)).sqrt());
        let scaled_width = unit_width * target;
        let scaled_height = unit_height * target;
        let rounded = EditorRenderSize::new(
            scaled_width.ceil().max(1.0) as u32,
            scaled_height.ceil().max(1.0) as u32,
        );
        // At a safety limit, round inward instead of exceeding the pixel budget.
        rounded.or_else(|_| {
            EditorRenderSize::new(
                scaled_width.floor().max(1.0) as u32,
                scaled_height.floor().max(1.0) as u32,
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorRenderSize {
    width: u32,
    height: u32,
}
impl EditorRenderSize {
    pub const MAX_PIXELS: u64 = ilikepdf_pdf::PdfPageRasterRequest::MAX_PIXELS;
    pub const MAX_AXIS: u32 = ilikepdf_pdf::PdfPageRasterRequest::MAX_AXIS;
    pub fn new(width: u32, height: u32) -> Result<Self, PageGeometryError> {
        if width == 0
            || height == 0
            || width > Self::MAX_AXIS
            || height > Self::MAX_AXIS
            || u64::from(width) * u64::from(height) > Self::MAX_PIXELS
        {
            return Err(PageGeometryError::InvalidDisplayRectangle);
        }
        Ok(Self { width, height })
    }
    pub fn width(self) -> u32 {
        self.width
    }
    pub fn height(self) -> u32 {
        self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorHit {
    pub page_index: u32,
    pub pdf_point: PdfPoint,
    pub page_local_point: ViewportPoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorDocumentLayout {
    pages: Vec<EditorPageLayout>,
    width: f64,
    height: f64,
    zoom: EditorZoom,
}

impl EditorDocumentLayout {
    pub const PADDING: f64 = 24.0;
    pub const PAGE_GAP: f64 = 24.0;
    pub fn new(
        session: &EditorSession,
        workspace_width: f64,
        mode: EditorZoomMode,
    ) -> Result<Self, PageGeometryError> {
        Self::from_pages(session.pages(), workspace_width, mode)
    }
    fn from_pages(
        metadata: &[EditorPageMetadata],
        workspace_width: f64,
        mode: EditorZoomMode,
    ) -> Result<Self, PageGeometryError> {
        if metadata.is_empty() || !workspace_width.is_finite() || workspace_width <= 0.0 {
            return Err(PageGeometryError::InvalidDisplayRectangle);
        }
        let widest = metadata
            .iter()
            .map(|p| p.geometry.display_width_points())
            .fold(0.0, f64::max);
        let zoom = match mode {
            EditorZoomMode::Custom(zoom) => zoom,
            EditorZoomMode::FitWidth => EditorZoom::new(
                ((workspace_width - 2.0 * Self::PADDING) / widest)
                    .clamp(EditorZoom::MIN, EditorZoom::MAX),
            )?,
        };
        let width = workspace_width.max(widest * zoom.scale() + 2.0 * Self::PADDING);
        let mut top = Self::PADDING;
        let mut pages = Vec::with_capacity(metadata.len());
        for page in metadata {
            let page_width = page.geometry.display_width_points() * zoom.scale();
            let transform = PageTransform::at_scale(
                page.geometry,
                ViewportPoint {
                    x: (width - page_width) / 2.0,
                    y: top,
                },
                zoom.scale(),
            )?;
            top += transform.display_rect().height() + Self::PAGE_GAP;
            pages.push(EditorPageLayout {
                page_index: page.page_index,
                geometry: page.geometry,
                transform,
                zoom,
            });
        }
        let height = top - Self::PAGE_GAP + Self::PADDING;
        if !height.is_finite() {
            return Err(PageGeometryError::InvalidDisplayRectangle);
        }
        Ok(Self {
            pages,
            width,
            height,
            zoom,
        })
    }
    pub fn pages(&self) -> &[EditorPageLayout] {
        &self.pages
    }
    pub fn width(&self) -> f64 {
        self.width
    }
    pub fn height(&self) -> f64 {
        self.height
    }
    pub fn zoom(&self) -> EditorZoom {
        self.zoom
    }

    /// Pointer is local to the workspace; scroll is the document-surface offset.
    pub fn hit_test(&self, point: ViewportPoint, scroll: ViewportPoint) -> Option<EditorHit> {
        let document = ViewportPoint {
            x: point.x + scroll.x,
            y: point.y + scroll.y,
        };
        if !document.x.is_finite() || !document.y.is_finite() {
            return None;
        }
        let index = self
            .pages
            .partition_point(|p| p.rect().top() + p.rect().height() < document.y);
        let page = self.pages.get(index)?;
        let pdf_point = page.transform.viewport_to_pdf(document).ok()?;
        Some(EditorHit {
            page_index: page.page_index,
            pdf_point,
            page_local_point: ViewportPoint {
                x: document.x - page.rect().left(),
                y: document.y - page.rect().top(),
            },
        })
    }

    /// No raster work occurs here. Binary search returns only pages near the viewport.
    pub fn visible_pages(&self, top: f64, height: f64, overscan: f64) -> Vec<u32> {
        if ![top, height, overscan].into_iter().all(f64::is_finite)
            || height <= 0.0
            || overscan < 0.0
        {
            return vec![];
        }
        let start = self
            .pages
            .partition_point(|p| p.rect().top() + p.rect().height() < top - overscan);
        self.pages[start..]
            .iter()
            .take_while(|p| p.rect().top() <= top + height + overscan)
            .map(|p| p.page_index)
            .collect()
    }

    /// Preserve the source point nearest the old viewport center. In a gap or
    /// margin, clamp to the nearest page edge. Bounds clamp at document ends.
    pub fn anchored_scroll(
        &self,
        previous: &Self,
        scroll: ViewportPoint,
        viewport: ViewportPoint,
    ) -> Result<ViewportPoint, PageGeometryError> {
        if ![scroll.x, scroll.y, viewport.x, viewport.y]
            .into_iter()
            .all(f64::is_finite)
            || scroll.x < 0.0
            || scroll.y < 0.0
            || viewport.x <= 0.0
            || viewport.y <= 0.0
        {
            return Err(PageGeometryError::InvalidDisplayRectangle);
        }
        let center = ViewportPoint {
            x: scroll.x + viewport.x / 2.0,
            y: scroll.y + viewport.y / 2.0,
        };
        let next = previous
            .pages
            .partition_point(|p| p.rect().top() + p.rect().height() < center.y);
        let candidates = [next.saturating_sub(1), next.min(previous.pages.len() - 1)];
        let page = candidates
            .into_iter()
            .map(|i| &previous.pages[i])
            .min_by(|a, b| {
                distance_y(a.rect(), center.y).total_cmp(&distance_y(b.rect(), center.y))
            })
            .expect("layouts are nonempty");
        let rect = page.rect();
        let clamped = ViewportPoint {
            x: center.x.clamp(rect.left(), rect.left() + rect.width()),
            y: center.y.clamp(rect.top(), rect.top() + rect.height()),
        };
        let point = page.transform.viewport_to_pdf(clamped)?;
        let target_page = self
            .pages
            .get(page.page_index as usize)
            .filter(|next| next.page_index == page.page_index && next.geometry == page.geometry)
            .ok_or(PageGeometryError::InvalidDisplayRectangle)?;
        let target = target_page.transform.pdf_to_viewport(point)?;
        Ok(ViewportPoint {
            x: (target.x - viewport.x / 2.0).clamp(0.0, (self.width - viewport.x).max(0.0)),
            y: (target.y - viewport.y / 2.0).clamp(0.0, (self.height - viewport.y).max(0.0)),
        })
    }
}

fn distance_y(rect: ViewportRect, y: f64) -> f64 {
    (rect.top() - y)
        .max(y - rect.top() - rect.height())
        .max(0.0)
}

#[cfg(test)]
mod tests;
