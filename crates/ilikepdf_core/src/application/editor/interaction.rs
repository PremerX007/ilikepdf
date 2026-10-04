use super::{
    EditorCommand, EditorEditError, EditorEditState, EditorObjectId, EditorRectangle,
    EditorSessionId, MIN_OBJECT_SIZE_POINTS, PageTransform, PdfPoint, ViewportPoint, ViewportRect,
};

/// Display-space handles; canonical edge ownership is resolved by PageTransform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}
impl EditorResizeHandle {
    pub const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Right,
        Self::BottomRight,
        Self::Bottom,
        Self::BottomLeft,
        Self::Left,
    ];
    pub fn point(self, rect: ViewportRect) -> ViewportPoint {
        let (x, y) = match self {
            Self::TopLeft => (0., 0.),
            Self::Top => (0.5, 0.),
            Self::TopRight => (1., 0.),
            Self::Right => (1., 0.5),
            Self::BottomRight => (1., 1.),
            Self::Bottom => (0.5, 1.),
            Self::BottomLeft => (0., 1.),
            Self::Left => (0., 0.5),
        };
        ViewportPoint {
            x: rect.left() + rect.width() * x,
            y: rect.top() + rect.height() * y,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorGestureKind {
    Move,
    Resize(EditorResizeHandle),
}

/// Frozen geometry transaction. Preview is pure and never touches the document/state/history.
pub struct EditorGesture {
    session_id: EditorSessionId,
    revision: u64,
    id: EditorObjectId,
    page_index: u32,
    before: EditorRectangle,
    transform: PageTransform,
    start: ViewportPoint,
    kind: EditorGestureKind,
    edges: [bool; 4], // canonical left, bottom, right, top
}
impl EditorRectangle {
    pub fn display_rect(self, transform: PageTransform) -> Result<ViewportRect, EditorEditError> {
        let b = self.bounds();
        let a = transform
            .pdf_to_viewport(PdfPoint {
                x: b.left_points(),
                y: b.bottom_points(),
            })
            .map_err(|_| EditorEditError::InvalidRectangle)?;
        let c = transform
            .pdf_to_viewport(PdfPoint {
                x: b.right_points(),
                y: b.top_points(),
            })
            .map_err(|_| EditorEditError::InvalidRectangle)?;
        ViewportRect::new(
            a.x.min(c.x),
            a.y.min(c.y),
            (a.x - c.x).abs(),
            (a.y - c.y).abs(),
        )
        .map_err(|_| EditorEditError::InvalidRectangle)
    }
}
impl EditorGesture {
    pub fn object_id(&self) -> EditorObjectId {
        self.id
    }
    pub fn page_index(&self) -> u32 {
        self.page_index
    }
    pub fn kind(&self) -> EditorGestureKind {
        self.kind
    }
    pub fn preview(&self, point: ViewportPoint) -> Result<EditorRectangle, EditorEditError> {
        let delta = self
            .transform
            .viewport_vector_to_pdf(ViewportPoint {
                x: point.x - self.start.x,
                y: point.y - self.start.y,
            })
            .map_err(|_| EditorEditError::InvalidPointer)?;
        let b = self.before.bounds();
        let page = self.transform.geometry().visible_box();
        let (mut l, mut d, mut r, mut t) = (
            b.left_points(),
            b.bottom_points(),
            b.right_points(),
            b.top_points(),
        );
        match self.kind {
            EditorGestureKind::Move => {
                let dx = delta
                    .x
                    .clamp(page.left_points() - l, page.right_points() - r);
                let dy = delta
                    .y
                    .clamp(page.bottom_points() - d, page.top_points() - t);
                l += dx;
                r += dx;
                d += dy;
                t += dy;
            }
            EditorGestureKind::Resize(_) => {
                let min_w = MIN_OBJECT_SIZE_POINTS.min(page.width_points());
                let min_h = MIN_OBJECT_SIZE_POINTS.min(page.height_points());
                if self.edges[0] {
                    l = (l + delta.x).clamp(page.left_points(), r - min_w);
                }
                if self.edges[1] {
                    d = (d + delta.y).clamp(page.bottom_points(), t - min_h);
                }
                if self.edges[2] {
                    r = (r + delta.x).clamp(l + min_w, page.right_points());
                }
                if self.edges[3] {
                    t = (t + delta.y).clamp(d + min_h, page.top_points());
                }
            }
        }
        let rectangle = EditorRectangle::new(l, d, r, t)?;
        rectangle.validate_on(page)?;
        Ok(rectangle)
    }
    pub fn preview_display(&self, point: ViewportPoint) -> Result<ViewportRect, EditorEditError> {
        self.preview(point)?.display_rect(self.transform)
    }
}
impl EditorEditState {
    pub fn hit_handle(
        &self,
        page_index: u32,
        transform: PageTransform,
        point: ViewportPoint,
    ) -> Result<Option<EditorResizeHandle>, EditorEditError> {
        let Some(object) = self
            .objects
            .iter()
            .find(|o| Some(o.id) == self.selected && o.page_index == page_index)
        else {
            return Ok(None);
        };
        let rect = object.rectangle.display_rect(transform)?;
        // Preserve a body target even when constant-sized handles overlap at low zoom.
        if (point.x - rect.left() - rect.width() / 2.0).abs() < rect.width() * 0.2
            && (point.y - rect.top() - rect.height() / 2.0).abs() < rect.height() * 0.2
        {
            return Ok(None);
        }
        Ok(EditorResizeHandle::ALL
            .into_iter()
            .filter(|h| {
                let p = h.point(rect);
                (p.x - point.x).abs() <= 6.0 && (p.y - point.y).abs() <= 6.0
            })
            .min_by(|a, b| {
                let distance = |h: EditorResizeHandle| {
                    let p = h.point(rect);
                    (p.x - point.x).powi(2) + (p.y - point.y).powi(2)
                };
                distance(*a).total_cmp(&distance(*b))
            }))
    }
    /// Handle hit testing precedes reverse-z body hit testing. Decoration stays logical-sized.
    pub fn begin_gesture(
        &mut self,
        page_index: u32,
        transform: PageTransform,
        point: ViewportPoint,
    ) -> Result<Option<EditorGesture>, EditorEditError> {
        let geometry = self
            .pages
            .get(page_index as usize)
            .ok_or(EditorEditError::InvalidPage)?
            .geometry;
        if transform.geometry() != geometry {
            return Err(EditorEditError::InvalidPage);
        }
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(EditorEditError::InvalidPointer);
        }
        let handle = self.hit_handle(page_index, transform, point)?;
        // A handle may extend a few logical units beyond the page edge.
        if handle.is_none() {
            let Ok(pdf) = transform.viewport_to_pdf(point) else {
                self.clear_selection();
                return Ok(None);
            };
            self.select_at(page_index, pdf);
        }
        let Some(object) = self
            .objects
            .iter()
            .find(|o| Some(o.id) == self.selected && o.page_index == page_index)
        else {
            return Ok(None);
        };
        let kind = handle.map_or(EditorGestureKind::Move, EditorGestureKind::Resize);
        let mut edges = [false; 4];
        if let Some(h) = handle {
            let p = transform
                .viewport_to_pdf(h.point(object.rectangle.display_rect(transform)?))
                .map_err(|_| EditorEditError::InvalidPointer)?;
            let b = object.rectangle.bounds();
            edges = [
                (p.x - b.left_points()).abs() < 1e-7,
                (p.y - b.bottom_points()).abs() < 1e-7,
                (p.x - b.right_points()).abs() < 1e-7,
                (p.y - b.top_points()).abs() < 1e-7,
            ];
        }
        Ok(Some(EditorGesture {
            session_id: self.session_id,
            revision: self.revision,
            id: object.id,
            page_index,
            before: object.rectangle,
            transform,
            start: point,
            kind,
            edges,
        }))
    }
    pub fn finish_gesture(
        &mut self,
        gesture: &EditorGesture,
        point: ViewportPoint,
    ) -> Result<bool, EditorEditError> {
        if self.session_id != gesture.session_id
            || self.revision != gesture.revision
            || !self
                .objects
                .iter()
                .any(|o| o.id == gesture.id && o.rectangle == gesture.before)
        {
            return Err(EditorEditError::StaleGesture);
        }
        let after = gesture.preview(point)?;
        if after == gesture.before {
            return Ok(false);
        }
        self.record(match gesture.kind {
            EditorGestureKind::Move => EditorCommand::MoveObject {
                id: gesture.id,
                before: gesture.before,
                after,
            },
            EditorGestureKind::Resize(_) => EditorCommand::ResizeObject {
                id: gesture.id,
                before: gesture.before,
                after,
            },
        });
        Ok(true)
    }
}
