use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::*;
use crate::{
    StructuralPdfEngineFamily, StructuralPdfEngineInfo, StructuralPdfError,
    StructuralPdfMergeRequest, StructuralPdfOperationResult, StructuralPdfPageRangeRequest,
    StructuralPdfValidation, StructuralPdfVersion,
};

struct FakeOrganizeEngine {
    page_plans: Mutex<Vec<StructuralPdfPagePlanRequest>>,
    fail_plan: bool,
}

impl FakeOrganizeEngine {
    fn succeeds() -> Self {
        Self {
            page_plans: Mutex::new(Vec::new()),
            fail_plan: false,
        }
    }

    fn fails() -> Self {
        Self {
            page_plans: Mutex::new(Vec::new()),
            fail_plan: true,
        }
    }
}

impl StructuralPdfEngine for FakeOrganizeEngine {
    fn probe(&self) -> Result<StructuralPdfEngineInfo, StructuralPdfError> {
        Ok(StructuralPdfEngineInfo {
            family: StructuralPdfEngineFamily::Qpdf,
            version: StructuralPdfVersion::new(12, 4, 1),
        })
    }

    fn validate(&self, _source_path: &Path) -> Result<StructuralPdfValidation, StructuralPdfError> {
        Ok(StructuralPdfValidation {
            has_warnings: false,
        })
    }

    fn rewrite(
        &self,
        _source_path: &Path,
        _working_output_path: &Path,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        unreachable!()
    }

    fn merge(
        &self,
        _request: &StructuralPdfMergeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        unreachable!()
    }

    fn create_page_range(
        &self,
        _request: &StructuralPdfPageRangeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        unreachable!()
    }

    fn create_page_plan(
        &self,
        request: &StructuralPdfPagePlanRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        self.page_plans.lock().unwrap().push(request.clone());
        if self.fail_plan {
            return Err(StructuralPdfError::OperationFailed);
        }
        fs::write(&request.working_output_path, b"organized fixture").unwrap();
        Ok(StructuralPdfOperationResult {
            has_warnings: false,
        })
    }
}

struct FakeVerifier {
    source_counts: HashMap<PathBuf, u32>,
    output_count: u32,
}

impl OrganizePdfVerifier for FakeVerifier {
    fn verify(&self, path: &Path) -> ApplicationResult<u32> {
        Ok(self
            .source_counts
            .get(path)
            .copied()
            .unwrap_or(self.output_count))
    }
}

fn source_info(path: &Path, page_count: u32) -> OrganizePdfSourceInfo {
    OrganizePdfSourceInfo {
        source_path: path.to_path_buf(),
        page_count,
        has_warnings: false,
    }
}

fn fixture_files() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("A.pdf");
    let second = directory.path().join("B.pdf");
    let third = directory.path().join("C.pdf");
    fs::write(&first, b"source A").unwrap();
    fs::write(&second, b"source B").unwrap();
    fs::write(&third, b"source C").unwrap();
    (directory, first, second, third)
}

#[test]
fn session_flattens_initial_and_later_sources_in_add_order() {
    let (_directory, first, second, third) = fixture_files();
    let mut session = OrganizePdfSession::new();

    session
        .add_sources(vec![source_info(&first, 2), source_info(&second, 1)])
        .unwrap();
    assert_eq!(
        session
            .page_items()
            .iter()
            .map(|page| (page.source_id, page.source_page_index))
            .collect::<Vec<_>>(),
        [(0, 0), (0, 1), (1, 0)]
    );
    session.add_sources(vec![source_info(&third, 2)]).unwrap();

    assert_eq!(
        session
            .page_items()
            .iter()
            .map(|page| (page.source_id, page.source_page_index))
            .collect::<Vec<_>>(),
        [(0, 0), (0, 1), (1, 0), (2, 0), (2, 1)]
    );
    assert_eq!(session.destination_directory(), first.parent());
    assert_eq!(session.output_name(), "organized.pdf");
}

#[test]
fn duplicate_addition_is_atomic_and_removing_a_source_removes_all_its_pages() {
    let (_directory, first, second, _third) = fixture_files();
    let mut session = OrganizePdfSession::new();
    session.add_sources(vec![source_info(&first, 2)]).unwrap();
    let before = session.clone();
    let equivalent_first = first.parent().unwrap().join(".").join("A.pdf");

    let error = session
        .add_sources(vec![
            source_info(&second, 1),
            source_info(&equivalent_first, 2),
        ])
        .unwrap_err();

    assert_eq!(error.code, ApplicationErrorCode::DuplicateSource);
    assert_eq!(session, before);
    session.add_sources(vec![source_info(&second, 1)]).unwrap();
    session.remove_source(0).unwrap();
    assert_eq!(session.sources().len(), 1);
    assert!(session.page_items().iter().all(|page| page.source_id == 1));
}

#[test]
fn page_edits_are_independent_and_reset_restores_current_sources() {
    let (_directory, first, second, _third) = fixture_files();
    let mut session = OrganizePdfSession::new();
    session
        .add_sources(vec![source_info(&first, 2), source_info(&second, 2)])
        .unwrap();
    let deleted_id = session.page_items()[1].page_item_id;
    let rotated_id = session.page_items()[2].page_item_id;

    session.reorder_page(2, 0).unwrap();
    session
        .rotate_page(rotated_id, PageRotationDirection::Right)
        .unwrap();
    session.delete_page(deleted_id).unwrap();
    assert_eq!(session.sources().len(), 2);
    assert_eq!(session.page_items().len(), 3);
    assert_eq!(
        session.page_items()[0].rotation,
        OrganizePdfPageRotation::Clockwise90
    );

    session.reset_all();
    assert_eq!(
        session
            .page_items()
            .iter()
            .map(|page| (page.source_id, page.source_page_index, page.rotation))
            .collect::<Vec<_>>(),
        [
            (0, 0, OrganizePdfPageRotation::None),
            (0, 1, OrganizePdfPageRotation::None),
            (1, 0, OrganizePdfPageRotation::None),
            (1, 1, OrganizePdfPageRotation::None),
        ]
    );
}

#[test]
fn rotation_cycles_in_quarter_turns_both_directions() {
    let (_directory, first, _second, _third) = fixture_files();
    let mut session = OrganizePdfSession::new();
    session.add_sources(vec![source_info(&first, 1)]).unwrap();
    let id = session.page_items()[0].page_item_id;

    for expected in [
        OrganizePdfPageRotation::Clockwise90,
        OrganizePdfPageRotation::HalfTurn,
        OrganizePdfPageRotation::CounterClockwise90,
        OrganizePdfPageRotation::None,
    ] {
        session
            .rotate_page(id, PageRotationDirection::Right)
            .unwrap();
        assert_eq!(session.page_items()[0].rotation, expected);
    }
    session
        .rotate_page(id, PageRotationDirection::Left)
        .unwrap();
    assert_eq!(
        session.page_items()[0].rotation,
        OrganizePdfPageRotation::CounterClockwise90
    );
}

#[test]
fn destination_and_empty_session_semantics_are_stable() {
    let (directory, first, second, _third) = fixture_files();
    let custom = directory.path().join("custom");
    fs::create_dir(&custom).unwrap();
    let mut session = OrganizePdfSession::new();
    assert!(!session.can_organize());
    session.add_sources(vec![source_info(&first, 1)]).unwrap();
    assert!(
        session.can_organize(),
        "one remaining page is a valid output"
    );
    session.set_custom_destination(custom.clone());
    session.add_sources(vec![source_info(&second, 1)]).unwrap();
    session.delete_page(0).unwrap();
    session.reset_all();
    assert_eq!(session.destination_directory(), Some(custom.as_path()));
    assert!(session.has_custom_destination());

    session.remove_source(0).unwrap();
    assert_eq!(session.destination_directory(), Some(custom.as_path()));
    session.remove_source(1).unwrap();
    assert_eq!(session.destination_directory(), None);
    assert!(!session.has_custom_destination());
    assert_eq!(session.output_name(), "organized.pdf");
    assert!(!session.can_organize());
}

#[test]
fn filename_normalization_accepts_names_and_rejects_paths() {
    assert_eq!(
        normalize_organize_pdf_output_name("report").unwrap(),
        "report.pdf"
    );
    assert_eq!(
        normalize_organize_pdf_output_name("report.pdf").unwrap(),
        "report.pdf"
    );
    for invalid in [
        "../report.pdf",
        r"..\report.pdf",
        r"C:\report.pdf",
        "NUL.pdf",
    ] {
        assert_eq!(
            normalize_organize_pdf_output_name(invalid)
                .unwrap_err()
                .code,
            ApplicationErrorCode::InvalidRequest
        );
    }
}

#[test]
fn execute_emits_the_exact_mixed_page_plan_and_publishes_once() {
    let (directory, first, second, third) = fixture_files();
    fs::write(directory.path().join("organized.PDF"), b"occupied").unwrap();
    let first_before = fs::read(&first).unwrap();
    let second_before = fs::read(&second).unwrap();
    let third_before = fs::read(&third).unwrap();
    let engine = FakeOrganizeEngine::succeeds();
    let verifier = FakeVerifier {
        source_counts: HashMap::from([(first.clone(), 2), (second.clone(), 2), (third.clone(), 1)]),
        output_count: 4,
    };
    let sources = vec![
        OrganizePdfSource {
            source_id: 10,
            source_path: first.clone(),
            page_count: 2,
            has_warnings: false,
        },
        OrganizePdfSource {
            source_id: 11,
            source_path: second.clone(),
            page_count: 2,
            has_warnings: false,
        },
        OrganizePdfSource {
            source_id: 12,
            source_path: third.clone(),
            page_count: 1,
            has_warnings: false,
        },
    ];
    let page_items = vec![
        OrganizePdfPageItem {
            page_item_id: 3,
            source_id: 11,
            source_page_index: 1,
            rotation: OrganizePdfPageRotation::None,
        },
        OrganizePdfPageItem {
            page_item_id: 0,
            source_id: 10,
            source_page_index: 0,
            rotation: OrganizePdfPageRotation::None,
        },
        OrganizePdfPageItem {
            page_item_id: 4,
            source_id: 12,
            source_page_index: 0,
            rotation: OrganizePdfPageRotation::Clockwise90,
        },
        OrganizePdfPageItem {
            page_item_id: 1,
            source_id: 10,
            source_page_index: 1,
            rotation: OrganizePdfPageRotation::CounterClockwise90,
        },
    ];
    let mut stages = Vec::new();

    let result = organize_pdf_with_verifier(
        &engine,
        &verifier,
        OrganizePdfRequest {
            sources,
            page_items,
            destination_directory: directory.path().to_path_buf(),
            output_name: "organized.pdf".to_owned(),
        },
        |progress| stages.push(progress.stage),
    )
    .unwrap();

    assert_eq!(
        result.output_path,
        directory.path().join("organized (1).pdf")
    );
    assert_eq!(result.source_count, 3);
    assert_eq!(result.page_count, 4);
    assert_eq!(
        stages,
        [
            OrganizePdfStage::Preparing,
            OrganizePdfStage::Organizing,
            OrganizePdfStage::Validating,
            OrganizePdfStage::Publishing,
            OrganizePdfStage::Completed,
        ]
    );
    let plans = engine.page_plans.lock().unwrap();
    assert_eq!(
        plans[0]
            .ordered_pages
            .iter()
            .map(|page| (&page.source_path, page.page_number, page.rotation))
            .collect::<Vec<_>>(),
        [
            (&second, 2, StructuralPdfPageRotation::None),
            (&first, 1, StructuralPdfPageRotation::None),
            (&third, 1, StructuralPdfPageRotation::Clockwise90),
            (&first, 2, StructuralPdfPageRotation::CounterClockwise90),
        ]
    );
    assert_eq!(fs::read(first).unwrap(), first_before);
    assert_eq!(fs::read(second).unwrap(), second_before);
    assert_eq!(fs::read(third).unwrap(), third_before);
}

#[test]
fn execute_rejects_zero_pages_and_duplicate_source_pages() {
    let (directory, first, _second, _third) = fixture_files();
    let source = OrganizePdfSource {
        source_id: 1,
        source_path: first.clone(),
        page_count: 2,
        has_warnings: false,
    };
    let verifier = FakeVerifier {
        source_counts: HashMap::from([(first, 2)]),
        output_count: 0,
    };
    let engine = FakeOrganizeEngine::succeeds();

    let empty = organize_pdf_with_verifier(
        &engine,
        &verifier,
        OrganizePdfRequest {
            sources: vec![source.clone()],
            page_items: Vec::new(),
            destination_directory: directory.path().to_path_buf(),
            output_name: "organized.pdf".to_owned(),
        },
        |_| {},
    )
    .unwrap_err();
    assert_eq!(empty.error.code, ApplicationErrorCode::InvalidOrganizePlan);

    let duplicate = organize_pdf_with_verifier(
        &engine,
        &verifier,
        OrganizePdfRequest {
            sources: vec![source],
            page_items: vec![
                OrganizePdfPageItem {
                    page_item_id: 0,
                    source_id: 1,
                    source_page_index: 0,
                    rotation: OrganizePdfPageRotation::None,
                },
                OrganizePdfPageItem {
                    page_item_id: 1,
                    source_id: 1,
                    source_page_index: 0,
                    rotation: OrganizePdfPageRotation::None,
                },
            ],
            destination_directory: directory.path().to_path_buf(),
            output_name: "organized.pdf".to_owned(),
        },
        |_| {},
    )
    .unwrap_err();
    assert_eq!(
        duplicate.error.code,
        ApplicationErrorCode::InvalidOrganizePlan
    );
    assert!(engine.page_plans.lock().unwrap().is_empty());
}

#[test]
fn engine_failure_is_all_or_nothing_and_keeps_sources_unchanged() {
    let (directory, first, _second, _third) = fixture_files();
    let before = fs::read(&first).unwrap();
    let engine = FakeOrganizeEngine::fails();
    let verifier = FakeVerifier {
        source_counts: HashMap::from([(first.clone(), 1)]),
        output_count: 1,
    };

    let failure = organize_pdf_with_verifier(
        &engine,
        &verifier,
        OrganizePdfRequest {
            sources: vec![OrganizePdfSource {
                source_id: 0,
                source_path: first.clone(),
                page_count: 1,
                has_warnings: false,
            }],
            page_items: vec![OrganizePdfPageItem {
                page_item_id: 0,
                source_id: 0,
                source_page_index: 0,
                rotation: OrganizePdfPageRotation::None,
            }],
            destination_directory: directory.path().to_path_buf(),
            output_name: "result.pdf".to_owned(),
        },
        |_| {},
    )
    .unwrap_err();

    assert_eq!(
        failure.error.code,
        ApplicationErrorCode::StructuralPdfOperationFailed
    );
    assert!(!directory.path().join("result.pdf").exists());
    assert_eq!(fs::read(first).unwrap(), before);
    assert_eq!(
        fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".ilikepdf-"))
            .count(),
        0
    );
}

#[test]
fn missing_source_at_execution_time_fails_without_publication() {
    let (directory, first, _second, _third) = fixture_files();
    fs::remove_file(&first).unwrap();
    let engine = FakeOrganizeEngine::succeeds();
    let verifier = FakeVerifier {
        source_counts: HashMap::new(),
        output_count: 1,
    };

    let failure = organize_pdf_with_verifier(
        &engine,
        &verifier,
        OrganizePdfRequest {
            sources: vec![OrganizePdfSource {
                source_id: 0,
                source_path: first,
                page_count: 1,
                has_warnings: false,
            }],
            page_items: vec![OrganizePdfPageItem {
                page_item_id: 0,
                source_id: 0,
                source_page_index: 0,
                rotation: OrganizePdfPageRotation::None,
            }],
            destination_directory: directory.path().to_path_buf(),
            output_name: "missing.pdf".to_owned(),
        },
        |_| {},
    )
    .unwrap_err();

    assert_eq!(failure.error.code, ApplicationErrorCode::SourceNotFound);
    assert!(!directory.path().join("missing.pdf").exists());
    assert!(engine.page_plans.lock().unwrap().is_empty());
}

#[test]
fn output_page_count_mismatch_removes_private_output() {
    let (directory, first, _second, _third) = fixture_files();
    let engine = FakeOrganizeEngine::succeeds();
    let verifier = FakeVerifier {
        source_counts: HashMap::from([(first.clone(), 1)]),
        output_count: 2,
    };

    let failure = organize_pdf_with_verifier(
        &engine,
        &verifier,
        OrganizePdfRequest {
            sources: vec![OrganizePdfSource {
                source_id: 0,
                source_path: first,
                page_count: 1,
                has_warnings: false,
            }],
            page_items: vec![OrganizePdfPageItem {
                page_item_id: 0,
                source_id: 0,
                source_page_index: 0,
                rotation: OrganizePdfPageRotation::None,
            }],
            destination_directory: directory.path().to_path_buf(),
            output_name: "mismatch.pdf".to_owned(),
        },
        |_| {},
    )
    .unwrap_err();

    assert_eq!(
        failure.error.code,
        ApplicationErrorCode::OrganizeOutputPageCountMismatch
    );
    assert!(!directory.path().join("mismatch.pdf").exists());
    assert_eq!(
        fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".ilikepdf-"))
            .count(),
        0
    );
}
