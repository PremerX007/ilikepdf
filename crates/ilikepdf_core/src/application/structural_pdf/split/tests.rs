use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::*;
use crate::{
    StructuralPdfEngineFamily, StructuralPdfEngineInfo, StructuralPdfValidation,
    StructuralPdfVersion,
};

struct FakeRangeEngine {
    sizes: HashMap<(u32, u32), u64>,
    requests: Mutex<Vec<StructuralPdfPageRangeRequest>>,
    fail_on_first_page: Option<u32>,
}

impl FakeRangeEngine {
    fn with_sizes(sizes: impl IntoIterator<Item = ((u32, u32), u64)>) -> Self {
        Self {
            sizes: sizes.into_iter().collect(),
            requests: Mutex::new(Vec::new()),
            fail_on_first_page: None,
        }
    }

    fn successful() -> Self {
        Self::with_sizes([])
    }
}

impl StructuralPdfEngine for FakeRangeEngine {
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
        Err(StructuralPdfError::OperationFailed)
    }

    fn merge(
        &self,
        _request: &super::super::StructuralPdfMergeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        Err(StructuralPdfError::OperationFailed)
    }

    fn create_page_range(
        &self,
        request: &StructuralPdfPageRangeRequest,
    ) -> Result<StructuralPdfOperationResult, StructuralPdfError> {
        self.requests.lock().unwrap().push(request.clone());
        if self.fail_on_first_page == Some(request.first_page) {
            return Err(StructuralPdfError::OperationFailed);
        }
        let page_count = request.last_page - request.first_page + 1;
        let size = self
            .sizes
            .get(&(request.first_page, request.last_page))
            .copied()
            .unwrap_or(u64::from(page_count).max(4));
        let mut output = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&request.working_output_path)
            .map_err(|_| StructuralPdfError::OutputWriteFailed)?;
        output
            .write_all(&page_count.to_le_bytes())
            .map_err(|_| StructuralPdfError::OutputWriteFailed)?;
        output
            .set_len(size.max(4))
            .map_err(|_| StructuralPdfError::OutputWriteFailed)?;
        Ok(StructuralPdfOperationResult {
            has_warnings: false,
        })
    }
}

struct EncodedPageCountVerifier;

impl SplitPdfVerifier for EncodedPageCountVerifier {
    fn verify(&self, path: &Path) -> ApplicationResult<u32> {
        let bytes = fs::read(path).map_err(|_| source_unreadable_error())?;
        let prefix: [u8; 4] = bytes
            .get(..4)
            .and_then(|value| value.try_into().ok())
            .ok_or_else(output_validation_error)?;
        Ok(u32::from_le_bytes(prefix))
    }
}

fn source_file(directory: &Path, name: &str, page_count: u32, size: u64) -> PathBuf {
    let path = directory.join(name);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)
        .unwrap();
    file.write_all(&page_count.to_le_bytes()).unwrap();
    file.set_len(size.max(4)).unwrap();
    path
}

fn run(
    engine: &FakeRangeEngine,
    source: PathBuf,
    destination: PathBuf,
    mode: SplitPdfMode,
) -> Result<SplitPdfResult, SplitPdfFailure> {
    split_pdf_with_verifier(
        engine,
        &EncodedPageCountVerifier,
        SplitPdfRequest {
            source_path: source,
            destination_directory: destination,
            mode,
        },
        |_| {},
    )
}

#[test]
fn every_page_derives_complete_ordered_ranges_and_names() {
    let directory = tempfile::tempdir().unwrap();
    let source = source_file(directory.path(), "report.pdf", 4, 100);
    let before = fs::read(&source).unwrap();

    let result = run(
        &FakeRangeEngine::successful(),
        source.clone(),
        directory.path().to_path_buf(),
        SplitPdfMode::EveryPage,
    )
    .unwrap();

    assert_eq!(result.parts.len(), 4);
    assert_eq!(
        result
            .parts
            .iter()
            .map(|part| (part.range.first_page, part.range.last_page))
            .collect::<Vec<_>>(),
        [(1, 1), (2, 2), (3, 3), (4, 4)]
    );
    assert_eq!(
        result.parts[0].output_path.file_name().unwrap(),
        "report-page-0001.pdf"
    );
    assert_eq!(fs::read(source).unwrap(), before);
}

#[test]
fn every_n_keeps_the_short_final_range_and_rejects_non_splits() {
    assert_eq!(
        plan_deterministic_ranges(10, &SplitPdfMode::EveryNPages { pages_per_part: 3 }).unwrap(),
        [
            SplitPdfPageRange {
                first_page: 1,
                last_page: 3
            },
            SplitPdfPageRange {
                first_page: 4,
                last_page: 6
            },
            SplitPdfPageRange {
                first_page: 7,
                last_page: 9
            },
            SplitPdfPageRange {
                first_page: 10,
                last_page: 10
            },
        ]
    );
    assert_eq!(
        plan_deterministic_ranges(10, &SplitPdfMode::EveryNPages { pages_per_part: 1 })
            .unwrap()
            .len(),
        10
    );
    for pages_per_part in [0, 10, 11] {
        assert!(
            plan_deterministic_ranges(10, &SplitPdfMode::EveryNPages { pages_per_part }).is_err()
        );
    }
}

#[test]
fn split_after_parser_preserves_user_order_and_rejects_bad_syntax() {
    assert_eq!(
        plan_deterministic_ranges(
            20,
            &SplitPdfMode::SplitAfterPages {
                split_after_pages: " 3, 7, 15 ".to_owned(),
            },
        )
        .unwrap(),
        [
            SplitPdfPageRange {
                first_page: 1,
                last_page: 3
            },
            SplitPdfPageRange {
                first_page: 4,
                last_page: 7
            },
            SplitPdfPageRange {
                first_page: 8,
                last_page: 15
            },
            SplitPdfPageRange {
                first_page: 16,
                last_page: 20
            },
        ]
    );
    for invalid in ["", "0", "20", "21", "7,3", "3,3", "3,,7", "abc", "3,abc,7"] {
        assert!(parse_split_points(invalid, 20).is_err(), "{invalid}");
    }
}

#[test]
fn maximum_size_uses_actual_candidates_and_accepts_exact_limit() {
    let directory = tempfile::tempdir().unwrap();
    let source = source_file(directory.path(), "sized.pdf", 5, 4_000_000);
    let engine = FakeRangeEngine::with_sizes([
        ((1, 1), 400_000),
        ((1, 2), 800_000),
        ((1, 3), 1_000_000),
        ((1, 4), 1_000_001),
        ((4, 4), 500_000),
        ((4, 5), 900_000),
    ]);

    let result = run(
        &engine,
        source,
        directory.path().to_path_buf(),
        SplitPdfMode::MaximumFileSize { maximum_size_mb: 1 },
    )
    .unwrap();

    assert_eq!(
        result
            .parts
            .iter()
            .map(|part| (part.range.first_page, part.range.last_page, part.size_bytes))
            .collect::<Vec<_>>(),
        [(1, 3, 1_000_000), (4, 5, 900_000)]
    );
    assert!(result.parts.iter().all(|part| part.size_bytes <= 1_000_000));
    assert!(
        engine
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|request| { request.first_page == 1 && request.last_page == 4 })
    );
}

#[test]
fn maximum_size_fails_whole_job_when_one_page_is_too_large() {
    let directory = tempfile::tempdir().unwrap();
    let source = source_file(directory.path(), "sized.pdf", 3, 4_000_000);
    let engine =
        FakeRangeEngine::with_sizes([((1, 1), 400_000), ((1, 2), 1_100_000), ((2, 2), 1_000_001)]);

    let failure = run(
        &engine,
        source,
        directory.path().to_path_buf(),
        SplitPdfMode::MaximumFileSize { maximum_size_mb: 1 },
    )
    .unwrap_err();

    assert_eq!(
        failure.error.code,
        ApplicationErrorCode::SplitPageExceedsSizeLimit
    );
    assert_eq!(failure.page_number, Some(2));
    assert_eq!(failure.actual_size_bytes, Some(1_000_001));
    assert!(!directory.path().join("sized-split").exists());
    assert!(
        fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".ilikepdf-split-pdf-"))
    );
}

#[test]
fn source_at_or_below_decimal_mb_limit_needs_no_split() {
    assert_eq!(10 * DECIMAL_BYTES_PER_MB, 10_000_000);
    let directory = tempfile::tempdir().unwrap();
    for size in [9_999_999, 10_000_000] {
        let source = source_file(directory.path(), &format!("source-{size}.pdf"), 2, size);
        let failure = run(
            &FakeRangeEngine::successful(),
            source,
            directory.path().to_path_buf(),
            SplitPdfMode::MaximumFileSize {
                maximum_size_mb: 10,
            },
        )
        .unwrap_err();
        assert_eq!(failure.error.code, ApplicationErrorCode::SplitNotRequired);
    }
}

#[test]
fn later_generation_failure_publishes_nothing_and_preserves_source() {
    let directory = tempfile::tempdir().unwrap();
    let source = source_file(directory.path(), "source.pdf", 3, 100);
    let before = fs::read(&source).unwrap();
    let mut engine = FakeRangeEngine::successful();
    engine.fail_on_first_page = Some(2);

    let failure = run(
        &engine,
        source.clone(),
        directory.path().to_path_buf(),
        SplitPdfMode::EveryPage,
    )
    .unwrap_err();

    assert_eq!(
        failure.error.code,
        ApplicationErrorCode::SplitCandidateGenerationFailed
    );
    assert!(!directory.path().join("source-split").exists());
    assert_eq!(fs::read(source).unwrap(), before);
}

#[test]
fn unicode_stems_and_large_numbers_expand_without_truncation() {
    assert_eq!(
        output_file_name(OsStr::new("รายงาน"), 10_000, true),
        OsString::from("รายงาน-page-10000.pdf")
    );
    assert_eq!(
        output_file_name(OsStr::new("รายงาน"), 12, false),
        OsString::from("รายงาน-part-0012.pdf")
    );
}

#[test]
fn one_page_source_is_rejected_without_creating_output() {
    let directory = tempfile::tempdir().unwrap();
    let source = source_file(directory.path(), "one.pdf", 1, 100);
    let failure = run(
        &FakeRangeEngine::successful(),
        source,
        directory.path().to_path_buf(),
        SplitPdfMode::EveryPage,
    )
    .unwrap_err();
    assert_eq!(failure.error.code, ApplicationErrorCode::PdfHasTooFewPages);
    assert!(!directory.path().join("one-split").exists());
}
