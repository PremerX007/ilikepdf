use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    StructuralPdfEngine, StructuralPdfPagePlanItem, StructuralPdfPagePlanRequest,
    StructuralPdfPageRotation,
};
use crate::application::output::PendingNumberedPdfOutput;
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfSourceInfo {
    pub source_path: PathBuf,
    pub page_count: u32,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfSource {
    pub source_id: u32,
    pub source_path: PathBuf,
    pub page_count: u32,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizePdfPageRotation {
    None,
    Clockwise90,
    HalfTurn,
    CounterClockwise90,
}

impl OrganizePdfPageRotation {
    fn rotate(self, direction: PageRotationDirection) -> Self {
        use OrganizePdfPageRotation::{Clockwise90, CounterClockwise90, HalfTurn, None};
        match (self, direction) {
            (None, PageRotationDirection::Right) | (HalfTurn, PageRotationDirection::Left) => {
                Clockwise90
            }
            (Clockwise90, PageRotationDirection::Right)
            | (CounterClockwise90, PageRotationDirection::Left) => HalfTurn,
            (HalfTurn, PageRotationDirection::Right) | (None, PageRotationDirection::Left) => {
                CounterClockwise90
            }
            (CounterClockwise90, PageRotationDirection::Right)
            | (Clockwise90, PageRotationDirection::Left) => None,
        }
    }
}

impl From<OrganizePdfPageRotation> for StructuralPdfPageRotation {
    fn from(value: OrganizePdfPageRotation) -> Self {
        match value {
            OrganizePdfPageRotation::None => Self::None,
            OrganizePdfPageRotation::Clockwise90 => Self::Clockwise90,
            OrganizePdfPageRotation::HalfTurn => Self::HalfTurn,
            OrganizePdfPageRotation::CounterClockwise90 => Self::CounterClockwise90,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRotationDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfPageItem {
    pub page_item_id: u32,
    pub source_id: u32,
    /// Zero-based page index in the source document.
    pub source_page_index: u32,
    pub rotation: OrganizePdfPageRotation,
}

/// Core-owned product state for one multi-source organize session.
///
/// Presentation code may mirror this model for immediate interaction, while
/// ingestion and execution still validate every invariant in core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfSession {
    sources: Vec<OrganizePdfSource>,
    page_items: Vec<OrganizePdfPageItem>,
    original_page_items: Vec<OrganizePdfPageItem>,
    destination_directory: Option<PathBuf>,
    has_custom_destination: bool,
    output_name: String,
    next_source_id: u32,
    next_page_item_id: u32,
}

impl Default for OrganizePdfSession {
    fn default() -> Self {
        Self::new()
    }
}

impl OrganizePdfSession {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            page_items: Vec::new(),
            original_page_items: Vec::new(),
            destination_directory: None,
            has_custom_destination: false,
            output_name: "organized.pdf".to_owned(),
            next_source_id: 0,
            next_page_item_id: 0,
        }
    }

    pub fn sources(&self) -> &[OrganizePdfSource] {
        &self.sources
    }

    pub fn page_items(&self) -> &[OrganizePdfPageItem] {
        &self.page_items
    }

    pub fn destination_directory(&self) -> Option<&Path> {
        self.destination_directory.as_deref()
    }

    pub fn has_custom_destination(&self) -> bool {
        self.has_custom_destination
    }

    pub fn output_name(&self) -> &str {
        &self.output_name
    }

    pub fn can_organize(&self) -> bool {
        !self.sources.is_empty()
            && !self.page_items.is_empty()
            && self.destination_directory.is_some()
            && normalize_organize_pdf_output_name(&self.output_name).is_ok()
    }

    pub fn add_sources(&mut self, additions: Vec<OrganizePdfSourceInfo>) -> ApplicationResult<()> {
        if additions.is_empty() {
            return Err(invalid_plan("Select at least one PDF to add"));
        }

        let mut identities = self
            .sources
            .iter()
            .map(|source| source_identity(&source.source_path))
            .collect::<ApplicationResult<HashSet<_>>>()?;
        for addition in &additions {
            validate_source_info(addition)?;
            if !identities.insert(source_identity(&addition.source_path)?) {
                return Err(duplicate_source_error());
            }
        }

        let first_source = self.sources.is_empty();
        if first_source && !self.has_custom_destination {
            self.destination_directory = additions
                .first()
                .and_then(|source| source.source_path.parent())
                .map(Path::to_path_buf);
        }

        for addition in additions {
            let source_id = self.next_source_id;
            self.next_source_id = self.next_source_id.checked_add(1).ok_or_else(|| {
                invalid_plan("Too many source PDFs were added to this organize session")
            })?;
            for source_page_index in 0..addition.page_count {
                let page_item_id = self.next_page_item_id;
                self.next_page_item_id =
                    self.next_page_item_id.checked_add(1).ok_or_else(|| {
                        invalid_plan("Too many pages were added to this organize session")
                    })?;
                let page = OrganizePdfPageItem {
                    page_item_id,
                    source_id,
                    source_page_index,
                    rotation: OrganizePdfPageRotation::None,
                };
                self.page_items.push(page.clone());
                self.original_page_items.push(page);
            }
            self.sources.push(OrganizePdfSource {
                source_id,
                source_path: addition.source_path,
                page_count: addition.page_count,
                has_warnings: addition.has_warnings,
            });
        }
        Ok(())
    }

    pub fn remove_source(&mut self, source_id: u32) -> ApplicationResult<()> {
        let source_index = self
            .sources
            .iter()
            .position(|source| source.source_id == source_id)
            .ok_or_else(|| invalid_plan("The selected source PDF is not in this session"))?;
        self.sources.remove(source_index);
        self.page_items.retain(|page| page.source_id != source_id);
        self.original_page_items
            .retain(|page| page.source_id != source_id);
        if self.sources.is_empty() {
            self.clear();
        }
        Ok(())
    }

    pub fn delete_page(&mut self, page_item_id: u32) -> ApplicationResult<()> {
        let index = self
            .page_items
            .iter()
            .position(|page| page.page_item_id == page_item_id)
            .ok_or_else(|| invalid_plan("The selected page is not in this organize plan"))?;
        self.page_items.remove(index);
        Ok(())
    }

    pub fn rotate_page(
        &mut self,
        page_item_id: u32,
        direction: PageRotationDirection,
    ) -> ApplicationResult<()> {
        let page = self
            .page_items
            .iter_mut()
            .find(|page| page.page_item_id == page_item_id)
            .ok_or_else(|| invalid_plan("The selected page is not in this organize plan"))?;
        page.rotation = page.rotation.rotate(direction);
        Ok(())
    }

    pub fn reorder_page(&mut self, old_index: usize, new_index: usize) -> ApplicationResult<()> {
        if old_index >= self.page_items.len() || new_index >= self.page_items.len() {
            return Err(invalid_plan("The selected page position is out of range"));
        }
        if old_index != new_index {
            let page = self.page_items.remove(old_index);
            self.page_items.insert(new_index, page);
        }
        Ok(())
    }

    pub fn reset_all(&mut self) {
        self.page_items.clone_from(&self.original_page_items);
    }

    pub fn set_output_name(&mut self, output_name: impl Into<String>) {
        self.output_name = output_name.into();
    }

    pub fn set_custom_destination(&mut self, destination_directory: PathBuf) {
        self.destination_directory = Some(destination_directory);
        self.has_custom_destination = true;
    }

    pub fn clear(&mut self) {
        self.sources.clear();
        self.page_items.clear();
        self.original_page_items.clear();
        self.destination_directory = None;
        self.has_custom_destination = false;
        self.output_name = "organized.pdf".to_owned();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfRequest {
    pub sources: Vec<OrganizePdfSource>,
    pub page_items: Vec<OrganizePdfPageItem>,
    pub destination_directory: PathBuf,
    pub output_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizePdfStage {
    Preparing,
    Organizing,
    Validating,
    Publishing,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrganizePdfProgress {
    pub stage: OrganizePdfStage,
    pub source_count: u32,
    pub page_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfResult {
    pub output_path: PathBuf,
    pub source_count: u32,
    pub page_count: u32,
    pub warning_source_count: u32,
    pub has_warnings: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizePdfFailure {
    pub error: ApplicationError,
    pub source_id: Option<u32>,
    pub source_path: Option<PathBuf>,
    pub page_item_id: Option<u32>,
    pub source_count: u32,
    pub page_count: u32,
}

impl OrganizePdfFailure {
    fn global(error: ApplicationError, source_count: u32, page_count: u32) -> Self {
        Self {
            error,
            source_id: None,
            source_path: None,
            page_item_id: None,
            source_count,
            page_count,
        }
    }

    fn for_source(
        error: ApplicationError,
        source: &OrganizePdfSource,
        source_count: u32,
        page_count: u32,
    ) -> Self {
        Self {
            error,
            source_id: Some(source.source_id),
            source_path: Some(source.source_path.clone()),
            page_item_id: None,
            source_count,
            page_count,
        }
    }

    fn for_page(
        error: ApplicationError,
        page_item_id: u32,
        source_count: u32,
        page_count: u32,
    ) -> Self {
        Self {
            error,
            source_id: None,
            source_path: None,
            page_item_id: Some(page_item_id),
            source_count,
            page_count,
        }
    }
}

pub fn inspect_organize_pdf_sources(
    engine: &dyn StructuralPdfEngine,
    existing_source_paths: &[PathBuf],
    candidate_source_paths: &[PathBuf],
) -> ApplicationResult<Vec<OrganizePdfSourceInfo>> {
    if candidate_source_paths.is_empty() {
        return Err(invalid_plan("Select at least one PDF to add"));
    }

    let mut identities = existing_source_paths
        .iter()
        .map(|path| source_identity_lexical(path))
        .collect::<ApplicationResult<HashSet<_>>>()?;
    for path in candidate_source_paths {
        validate_source(path)?;
        let identity = source_identity(path)?;
        if !identities.insert(identity) {
            return Err(duplicate_source_error());
        }
    }

    // Candidate addition is deliberately all-or-nothing: build the complete
    // validated result before returning anything to the presentation layer.
    candidate_source_paths
        .iter()
        .map(|source_path| {
            let validation = engine
                .validate(source_path)
                .map_err(map_organize_engine_error)?;
            let info =
                ilikepdf_pdf::inspect_document(source_path).map_err(ApplicationError::from)?;
            if info.page_count == 0 {
                return Err(invalid_plan("The selected PDF does not contain any pages"));
            }
            Ok(OrganizePdfSourceInfo {
                source_path: std::path::absolute(source_path).map_err(|_| {
                    ApplicationError::new(
                        ApplicationErrorCode::SourceUnreadable,
                        "The selected PDF could not be read",
                    )
                })?,
                page_count: info.page_count,
                has_warnings: validation.has_warnings,
            })
        })
        .collect()
}

pub fn normalize_organize_pdf_output_name(output_name: &str) -> ApplicationResult<String> {
    crate::application::output::normalize_pdf_filename(output_name)
        .map(|name| name.to_string_lossy().into_owned())
}

pub fn organize_pdf(
    engine: &dyn StructuralPdfEngine,
    request: OrganizePdfRequest,
    on_progress: impl FnMut(OrganizePdfProgress),
) -> Result<OrganizePdfResult, OrganizePdfFailure> {
    organize_pdf_with_verifier(engine, &NativeOrganizePdfVerifier, request, on_progress)
}

fn organize_pdf_with_verifier(
    engine: &dyn StructuralPdfEngine,
    verifier: &impl OrganizePdfVerifier,
    request: OrganizePdfRequest,
    mut on_progress: impl FnMut(OrganizePdfProgress),
) -> Result<OrganizePdfResult, OrganizePdfFailure> {
    let source_count = u32::try_from(request.sources.len()).map_err(|_| {
        OrganizePdfFailure::global(
            invalid_plan("Too many source PDFs were added to this organize session"),
            u32::MAX,
            0,
        )
    })?;
    let page_count = u32::try_from(request.page_items.len()).map_err(|_| {
        OrganizePdfFailure::global(
            invalid_plan("Too many pages were added to this organize session"),
            source_count,
            u32::MAX,
        )
    })?;
    emit_progress(
        &mut on_progress,
        OrganizePdfStage::Preparing,
        source_count,
        page_count,
    );
    if source_count == 0 {
        return Err(OrganizePdfFailure::global(
            invalid_plan("Add at least one source PDF before organizing"),
            source_count,
            page_count,
        ));
    }
    if page_count == 0 {
        return Err(OrganizePdfFailure::global(
            invalid_plan("Keep at least one page in the organize plan"),
            source_count,
            page_count,
        ));
    }

    let pending = PendingNumberedPdfOutput::in_directory(
        &request.destination_directory,
        &request.output_name,
    )
    .map_err(|error| OrganizePdfFailure::global(error, source_count, page_count))?;

    let mut sources_by_id = HashMap::with_capacity(request.sources.len());
    let mut source_identities = HashSet::with_capacity(request.sources.len());
    let mut warning_source_count = 0_u32;
    for source in &request.sources {
        if sources_by_id.insert(source.source_id, source).is_some() {
            return Err(OrganizePdfFailure::for_source(
                invalid_plan("The organize source list contains a duplicate identifier"),
                source,
                source_count,
                page_count,
            ));
        }
        validate_source(&source.source_path).map_err(|error| {
            OrganizePdfFailure::for_source(error, source, source_count, page_count)
        })?;
        let identity = source_identity(&source.source_path).map_err(|error| {
            OrganizePdfFailure::for_source(error, source, source_count, page_count)
        })?;
        if !source_identities.insert(identity) {
            return Err(OrganizePdfFailure::for_source(
                duplicate_source_error(),
                source,
                source_count,
                page_count,
            ));
        }
        let validation = engine.validate(&source.source_path).map_err(|error| {
            OrganizePdfFailure::for_source(
                map_organize_engine_error(error),
                source,
                source_count,
                page_count,
            )
        })?;
        let actual_page_count = verifier.verify(&source.source_path).map_err(|error| {
            OrganizePdfFailure::for_source(error, source, source_count, page_count)
        })?;
        if actual_page_count != source.page_count {
            return Err(OrganizePdfFailure::for_source(
                invalid_plan("A source PDF changed after it was added to the organize session"),
                source,
                source_count,
                page_count,
            ));
        }
        if source.has_warnings || validation.has_warnings {
            warning_source_count += 1;
        }
    }

    let mut page_item_ids = HashSet::with_capacity(request.page_items.len());
    let mut source_pages = HashSet::with_capacity(request.page_items.len());
    let mut used_source_ids = HashSet::with_capacity(request.sources.len());
    let mut structural_pages = Vec::with_capacity(request.page_items.len());
    for page in &request.page_items {
        if !page_item_ids.insert(page.page_item_id) {
            return Err(OrganizePdfFailure::for_page(
                invalid_plan("The organize plan contains a duplicate page item"),
                page.page_item_id,
                source_count,
                page_count,
            ));
        }
        let source = sources_by_id.get(&page.source_id).ok_or_else(|| {
            OrganizePdfFailure::for_page(
                invalid_plan("The organize plan references a source that is not loaded"),
                page.page_item_id,
                source_count,
                page_count,
            )
        })?;
        if page.source_page_index >= source.page_count {
            return Err(OrganizePdfFailure::for_page(
                invalid_plan("The organize plan references a page outside its source PDF"),
                page.page_item_id,
                source_count,
                page_count,
            ));
        }
        if !source_pages.insert((page.source_id, page.source_page_index)) {
            return Err(OrganizePdfFailure::for_page(
                invalid_plan("Duplicating pages is not supported in Organize PDF"),
                page.page_item_id,
                source_count,
                page_count,
            ));
        }
        used_source_ids.insert(page.source_id);
        structural_pages.push(StructuralPdfPagePlanItem {
            source_path: source.source_path.clone(),
            page_number: page.source_page_index + 1,
            rotation: page.rotation.into(),
        });
    }

    emit_progress(
        &mut on_progress,
        OrganizePdfStage::Organizing,
        source_count,
        page_count,
    );
    let operation = engine
        .create_page_plan(&StructuralPdfPagePlanRequest {
            ordered_pages: structural_pages,
            working_output_path: pending.working_path().to_path_buf(),
        })
        .map_err(|error| {
            OrganizePdfFailure::global(map_organize_engine_error(error), source_count, page_count)
        })?;

    emit_progress(
        &mut on_progress,
        OrganizePdfStage::Validating,
        source_count,
        page_count,
    );
    let metadata = fs::metadata(pending.working_path()).map_err(|_| {
        OrganizePdfFailure::global(output_validation_error(), source_count, page_count)
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(OrganizePdfFailure::global(
            output_validation_error(),
            source_count,
            page_count,
        ));
    }
    let output_validation = engine.validate(pending.working_path()).map_err(|_| {
        OrganizePdfFailure::global(output_validation_error(), source_count, page_count)
    })?;
    let output_page_count = verifier.verify(pending.working_path()).map_err(|_| {
        OrganizePdfFailure::global(
            ApplicationError::new(
                ApplicationErrorCode::StructuralPdfOutputValidationFailed,
                "The organized PDF could not be reopened",
            ),
            source_count,
            page_count,
        )
    })?;
    if output_page_count != page_count {
        return Err(OrganizePdfFailure::global(
            ApplicationError::new(
                ApplicationErrorCode::OrganizeOutputPageCountMismatch,
                "The organized PDF did not contain the expected number of pages",
            ),
            source_count,
            page_count,
        ));
    }

    emit_progress(
        &mut on_progress,
        OrganizePdfStage::Publishing,
        source_count,
        page_count,
    );
    let output_path = pending
        .publish()
        .map_err(|error| OrganizePdfFailure::global(error, source_count, output_page_count))?;
    let used_source_count = u32::try_from(used_source_ids.len()).unwrap_or(u32::MAX);
    emit_progress(
        &mut on_progress,
        OrganizePdfStage::Completed,
        used_source_count,
        output_page_count,
    );
    Ok(OrganizePdfResult {
        output_path,
        source_count: used_source_count,
        page_count: output_page_count,
        warning_source_count,
        has_warnings: warning_source_count > 0
            || operation.has_warnings
            || output_validation.has_warnings,
    })
}

trait OrganizePdfVerifier {
    fn verify(&self, path: &Path) -> ApplicationResult<u32>;
}

struct NativeOrganizePdfVerifier;

impl OrganizePdfVerifier for NativeOrganizePdfVerifier {
    fn verify(&self, path: &Path) -> ApplicationResult<u32> {
        ilikepdf_pdf::inspect_document(path)
            .map(|info| info.page_count)
            .map_err(ApplicationError::from)
    }
}

fn validate_source_info(source: &OrganizePdfSourceInfo) -> ApplicationResult<()> {
    validate_source(&source.source_path)?;
    if source.page_count == 0 {
        return Err(invalid_plan("The selected PDF does not contain any pages"));
    }
    Ok(())
}

fn validate_source(path: &Path) -> ApplicationResult<()> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(ApplicationError::new(
            ApplicationErrorCode::SourceNotFile,
            "The selected PDF is not a file",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(ApplicationError::new(
            ApplicationErrorCode::SourceNotFound,
            "The selected PDF no longer exists",
        )),
        Err(_) => Err(ApplicationError::new(
            ApplicationErrorCode::SourceUnreadable,
            "The selected PDF could not be read",
        )),
    }
}

fn source_identity(path: &Path) -> ApplicationResult<String> {
    fs::canonicalize(path)
        .map_err(|_| {
            ApplicationError::new(
                ApplicationErrorCode::SourceUnreadable,
                "The selected PDF could not be read",
            )
        })
        .map(|path| path_identity_key(&path))
}

fn source_identity_lexical(path: &Path) -> ApplicationResult<String> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(path_identity_key(&path)),
        Err(_) => std::path::absolute(path)
            .map(|path| path_identity_key(&path))
            .map_err(|_| {
                ApplicationError::new(
                    ApplicationErrorCode::SourceUnreadable,
                    "An existing source path could not be resolved",
                )
            }),
    }
}

fn path_identity_key(path: &Path) -> String {
    let value = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

fn duplicate_source_error() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::DuplicateSource,
        "This PDF has already been added to the organize session",
    )
}

fn invalid_plan(message: &'static str) -> ApplicationError {
    ApplicationError::new(ApplicationErrorCode::InvalidOrganizePlan, message)
}

fn output_validation_error() -> ApplicationError {
    ApplicationError::new(
        ApplicationErrorCode::StructuralPdfOutputValidationFailed,
        "The organized PDF failed validation",
    )
}

fn map_organize_engine_error(error: super::StructuralPdfError) -> ApplicationError {
    use super::StructuralPdfError;
    match error {
        StructuralPdfError::RuntimeUnavailable => ApplicationError::new(
            ApplicationErrorCode::StructuralPdfRuntimeUnavailable,
            "The bundled structural PDF runtime is unavailable",
        ),
        StructuralPdfError::RuntimeIncompatible => ApplicationError::new(
            ApplicationErrorCode::StructuralPdfRuntimeIncompatible,
            "The bundled structural PDF runtime is incompatible",
        ),
        StructuralPdfError::RuntimeLaunchFailed => ApplicationError::new(
            ApplicationErrorCode::StructuralPdfLaunchFailed,
            "The structural PDF runtime could not be started",
        ),
        StructuralPdfError::SourceNotFound => ApplicationError::new(
            ApplicationErrorCode::SourceNotFound,
            "A source PDF no longer exists",
        ),
        StructuralPdfError::SourceNotFile => ApplicationError::new(
            ApplicationErrorCode::SourceNotFile,
            "A source PDF is not a file",
        ),
        StructuralPdfError::SourceUnreadable => ApplicationError::new(
            ApplicationErrorCode::SourceUnreadable,
            "A source PDF could not be read",
        ),
        StructuralPdfError::PasswordRequired => ApplicationError::new(
            ApplicationErrorCode::PasswordRequired,
            "This PDF is password protected. Unlock it before organizing",
        ),
        StructuralPdfError::InvalidDocument => ApplicationError::new(
            ApplicationErrorCode::InvalidPdf,
            "The selected file is not a structurally valid PDF",
        ),
        StructuralPdfError::OutputWriteFailed => ApplicationError::new(
            ApplicationErrorCode::OutputWriteFailed,
            "The organized PDF could not be written",
        ),
        StructuralPdfError::OperationFailed => ApplicationError::new(
            ApplicationErrorCode::StructuralPdfOperationFailed,
            "The PDF pages could not be organized",
        ),
    }
}

fn emit_progress(
    on_progress: &mut impl FnMut(OrganizePdfProgress),
    stage: OrganizePdfStage,
    source_count: u32,
    page_count: u32,
) {
    on_progress(OrganizePdfProgress {
        stage,
        source_count,
        page_count,
    });
}

#[cfg(test)]
mod tests;
