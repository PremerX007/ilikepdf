use std::collections::VecDeque;

use super::{EditorPageMetadata, EditorSession, EditorSessionId, PageBox, PdfPoint};

pub const EDITOR_HISTORY_LIMIT: usize = 100;
pub const EDITOR_OBJECT_LIMIT: usize = 1000;
pub const MIN_OBJECT_SIZE_POINTS: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorEditError {
    InvalidRectangle,
    InvalidPage,
    StaleGesture,
    InvalidPointer,
    CapacityReached,
}

/// A positive finite rectangle in unrotated source PDF points, never pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorRectangle(PageBox);

impl EditorRectangle {
    pub fn new(left: f64, bottom: f64, right: f64, top: f64) -> Result<Self, EditorEditError> {
        PageBox::new(left, bottom, right, top)
            .map(Self)
            .map_err(|_| EditorEditError::InvalidRectangle)
    }
    pub fn bounds(self) -> PageBox {
        self.0
    }
    pub fn contains(self, point: PdfPoint) -> bool {
        point.x >= self.0.left_points()
            && point.x <= self.0.right_points()
            && point.y >= self.0.bottom_points()
            && point.y <= self.0.top_points()
    }
    pub(super) fn validate_on(self, page: PageBox) -> Result<(), EditorEditError> {
        let b = self.0;
        if b.left_points() < page.left_points()
            || b.right_points() > page.right_points()
            || b.bottom_points() < page.bottom_points()
            || b.top_points() > page.top_points()
            || b.width_points() < MIN_OBJECT_SIZE_POINTS.min(page.width_points())
            || b.height_points() < MIN_OBJECT_SIZE_POINTS.min(page.height_points())
        {
            return Err(EditorEditError::InvalidRectangle);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorObjectId(u64);
impl EditorObjectId {
    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorObjectKind {
    PrototypeRectangle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorObject {
    pub(super) id: EditorObjectId,
    pub(super) page_index: u32,
    pub(super) kind: EditorObjectKind,
    pub(super) rectangle: EditorRectangle,
}
impl EditorObject {
    pub fn id(&self) -> EditorObjectId {
        self.id
    }
    pub fn page_index(&self) -> u32 {
        self.page_index
    }
    pub fn kind(&self) -> EditorObjectKind {
        self.kind
    }
    pub fn rectangle(&self) -> EditorRectangle {
        self.rectangle
    }
}

/// Small inverse-capable edits. Object insertion/deletion preserve stacking slots.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorCommand {
    AddObject {
        object: EditorObject,
        position: usize,
    },
    MoveObject {
        id: EditorObjectId,
        before: EditorRectangle,
        after: EditorRectangle,
    },
    ResizeObject {
        id: EditorObjectId,
        before: EditorRectangle,
        after: EditorRectangle,
    },
    DeleteObject {
        object: EditorObject,
        position: usize,
    },
}

/// In-memory state scoped to one immutable source session. No native handles or writer.
pub struct EditorEditState {
    pub(super) session_id: EditorSessionId,
    pub(super) pages: Vec<EditorPageMetadata>,
    pub(super) objects: Vec<EditorObject>,
    pub(super) selected: Option<EditorObjectId>,
    undo: VecDeque<EditorCommand>,
    redo: Vec<EditorCommand>,
    next_id: u64,
    pub(super) revision: u64,
}
impl EditorEditState {
    pub fn new(session: &EditorSession) -> Self {
        Self {
            session_id: session.id(),
            pages: session.pages().to_vec(),
            objects: vec![],
            selected: None,
            undo: VecDeque::new(),
            redo: vec![],
            next_id: 1,
            revision: 0,
        }
    }
    pub fn session_id(&self) -> EditorSessionId {
        self.session_id
    }
    pub fn objects(&self) -> &[EditorObject] {
        &self.objects
    }
    pub fn selected(&self) -> Option<EditorObjectId> {
        self.selected
    }
    pub fn undo_count(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_count(&self) -> usize {
        self.redo.len()
    }
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }
    pub fn select_at(&mut self, page_index: u32, point: PdfPoint) -> Option<EditorObjectId> {
        self.selected = self
            .objects
            .iter()
            .rev()
            .find(|o| o.page_index == page_index && o.rectangle.contains(point))
            .map(|o| o.id);
        self.selected
    }
    pub fn add_prototype(&mut self, page_index: u32) -> Result<EditorObjectId, EditorEditError> {
        let b = self.page_box(page_index)?;
        let w = (b.width_points() * 0.25)
            .max(MIN_OBJECT_SIZE_POINTS)
            .min(b.width_points());
        let h = (b.height_points() * 0.2)
            .max(MIN_OBJECT_SIZE_POINTS)
            .min(b.height_points());
        let left = b.left_points() + (b.width_points() - w) / 2.0;
        let bottom = b.bottom_points() + (b.height_points() - h) / 2.0;
        self.insert_prototype(
            page_index,
            EditorRectangle::new(left, bottom, left + w, bottom + h)?,
        )
    }
    pub fn insert_prototype(
        &mut self,
        page_index: u32,
        rectangle: EditorRectangle,
    ) -> Result<EditorObjectId, EditorEditError> {
        rectangle.validate_on(self.page_box(page_index)?)?;
        if self.objects.len() >= EDITOR_OBJECT_LIMIT {
            return Err(EditorEditError::CapacityReached);
        }
        let id = EditorObjectId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(EditorEditError::CapacityReached)?;
        let object = EditorObject {
            id,
            page_index,
            kind: EditorObjectKind::PrototypeRectangle,
            rectangle,
        };
        self.record(EditorCommand::AddObject {
            object,
            position: self.objects.len(),
        });
        self.selected = Some(id);
        Ok(id)
    }
    pub fn delete_selected(&mut self) -> bool {
        let Some(position) = self
            .objects
            .iter()
            .position(|o| Some(o.id) == self.selected)
        else {
            return false;
        };
        self.record(EditorCommand::DeleteObject {
            object: self.objects[position].clone(),
            position,
        });
        true
    }
    pub fn undo(&mut self) -> bool {
        let Some(command) = self.undo.pop_back() else {
            return false;
        };
        self.apply(&command, false);
        self.redo.push(command);
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(command) = self.redo.pop() else {
            return false;
        };
        self.apply(&command, true);
        self.undo.push_back(command);
        true
    }
    pub(super) fn page_box(&self, index: u32) -> Result<PageBox, EditorEditError> {
        self.pages
            .get(index as usize)
            .map(|p| p.geometry.visible_box())
            .ok_or(EditorEditError::InvalidPage)
    }
    pub(super) fn record(&mut self, command: EditorCommand) {
        self.apply(&command, true);
        self.redo.clear();
        self.undo.push_back(command);
        if self.undo.len() > EDITOR_HISTORY_LIMIT {
            self.undo.pop_front();
        }
    }
    fn apply(&mut self, command: &EditorCommand, forward: bool) {
        match command {
            EditorCommand::AddObject { object, position }
            | EditorCommand::DeleteObject { object, position } => {
                let insert = matches!(command, EditorCommand::AddObject { .. }) == forward;
                if insert {
                    self.objects.insert(*position, object.clone());
                } else {
                    self.objects.remove(*position);
                }
            }
            EditorCommand::MoveObject { id, before, after }
            | EditorCommand::ResizeObject { id, before, after } => {
                // Commands are constructed only against this state's live identity.
                let object = self
                    .objects
                    .iter_mut()
                    .find(|o| o.id == *id)
                    .expect("history object exists");
                object.rectangle = if forward { *after } else { *before };
            }
        }
        if !self.objects.iter().any(|o| Some(o.id) == self.selected) {
            self.selected = None;
        }
        self.revision += 1;
    }
}

#[cfg(test)]
mod tests;
