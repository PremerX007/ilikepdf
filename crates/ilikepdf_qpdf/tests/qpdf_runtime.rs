use std::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Write as _};
use std::path::{Path, PathBuf};

use ilikepdf_core::{
    ApplicationErrorCode, MergePdfRequest, RewriteStructuralPdfRequest, SplitPdfMode,
    SplitPdfRequest, StructuralPdfEngine, StructuralPdfEngineFamily, StructuralPdfError,
    StructuralPdfMergeRequest, StructuralPdfVersion, merge_pdf, probe_structural_pdf_engine,
    rewrite_structural_pdf, split_pdf, validate_structural_pdf,
};
use ilikepdf_pdf::{PdfRenderRequest, inspect_document, render_page_to_png};
use ilikepdf_qpdf::QpdfCliEngine;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn qpdf_runtime_root() -> PathBuf {
    repository_root()
        .join("third_party")
        .join("qpdf")
        .join("windows")
        .join("x64")
}

fn pdfium_library() -> PathBuf {
    repository_root()
        .join("third_party")
        .join("pdfium")
        .join("windows")
        .join("x64")
        .join("pdfium.dll")
}

fn fixture(name: &str) -> PathBuf {
    repository_root()
        .join("crates")
        .join("ilikepdf_pdf")
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn official_runtime_probe_returns_the_pinned_typed_version() {
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());

    let info = probe_structural_pdf_engine(&engine).expect("vendored qpdf should be available");

    assert_eq!(info.family, StructuralPdfEngineFamily::Qpdf);
    assert_eq!(info.version, StructuralPdfVersion::new(12, 4, 1));
}

#[test]
fn real_qpdf_validation_accepts_valid_pdf_and_rejects_malformed_input() {
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());
    let valid_source = fixture("one_page.pdf");
    let malformed_source = fixture("malformed.pdf");
    let valid_before = fs::read(&valid_source).expect("valid fixture should be readable");
    let malformed_before =
        fs::read(&malformed_source).expect("malformed fixture should be readable");

    let validation = validate_structural_pdf(&engine, &valid_source)
        .expect("qpdf should accept the valid fixture");
    let error = validate_structural_pdf(&engine, &malformed_source)
        .expect_err("qpdf should reject malformed input");

    assert!(!validation.has_warnings);
    assert_eq!(error.code, ApplicationErrorCode::InvalidPdf);
    assert_eq!(fs::read(valid_source).unwrap(), valid_before);
    assert_eq!(fs::read(malformed_source).unwrap(), malformed_before);
}

#[test]
fn application_rewrite_handles_spaces_and_unicode_and_round_trips_through_pdfium() {
    install_pdfium_beside_test_executable();
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let unicode_directory = directory.path().join("พื้นที่ ทดสอบ qpdf");
    fs::create_dir(&unicode_directory).expect("Unicode test directory should be created");
    let source = unicode_directory.join("ต้นฉบับ สองหน้า.pdf");
    let destination = unicode_directory.join("ผลลัพธ์ เขียนใหม่.pdf");
    fs::copy(fixture("two_page.pdf"), &source).expect("fixture should be copied");
    let source_before = fs::read(&source).expect("source should be readable");
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());

    let result = rewrite_structural_pdf(
        &engine,
        RewriteStructuralPdfRequest {
            source_path: source.clone(),
            destination_path: destination.clone(),
        },
    )
    .expect("content-preserving rewrite should succeed");

    assert_eq!(result.output_path, destination);
    assert_eq!(result.page_count, 2);
    assert!(!result.has_warnings);
    assert!(result.output_path.metadata().unwrap().len() > 0);
    assert_eq!(fs::read(&source).unwrap(), source_before);
    assert_eq!(private_working_file_count(&unicode_directory), 0);

    let output_info = inspect_document(&result.output_path)
        .expect("rewritten output should reopen through PDFium");
    let mut rendered = Cursor::new(Vec::new());
    render_page_to_png(
        PdfRenderRequest {
            source_path: result.output_path,
            page_index: 0,
            target_width: 128,
        },
        &mut rendered,
    )
    .expect("a representative rewritten page should render");

    assert_eq!(output_info.page_count, 2);
    assert_eq!(&rendered.into_inner()[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn existing_destination_is_not_replaced_and_leaves_no_working_file() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let destination = directory.path().join("existing.pdf");
    fs::write(&destination, b"existing destination").unwrap();
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());

    let error = rewrite_structural_pdf(
        &engine,
        RewriteStructuralPdfRequest {
            source_path: fixture("one_page.pdf"),
            destination_path: destination.clone(),
        },
    )
    .expect_err("existing output must not be overwritten");

    assert_eq!(error.code, ApplicationErrorCode::OutputAlreadyExists);
    assert_eq!(fs::read(destination).unwrap(), b"existing destination");
    assert_eq!(private_working_file_count(directory.path()), 0);
}

#[test]
fn missing_bundled_runtime_maps_without_falling_back_to_path() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let engine = QpdfCliEngine::from_runtime_root(directory.path().join("absent"));

    assert_eq!(engine.probe(), Err(StructuralPdfError::RuntimeUnavailable));
    let error = probe_structural_pdf_engine(&engine)
        .expect_err("the application boundary should map the infrastructure error");
    assert_eq!(
        error.code,
        ApplicationErrorCode::StructuralPdfRuntimeUnavailable
    );
}

#[test]
fn real_merge_preserves_order_duplicates_geometry_sources_and_collision_safety() {
    install_pdfium_beside_test_executable();
    let directory = tempfile::tempdir().unwrap();
    let source_directory = directory.path().join("PDF sources with spaces พื้นที่");
    let destination_directory = directory.path().join("custom output");
    fs::create_dir(&source_directory).unwrap();
    fs::create_dir(&destination_directory).unwrap();
    let first = source_directory.join("A สองหน้า.pdf");
    let second = source_directory.join("B one page.pdf");
    fs::copy(fixture("two_page.pdf"), &first).unwrap();
    fs::copy(fixture("one_page.pdf"), &second).unwrap();
    let first_before = fs::read(&first).unwrap();
    let second_before = fs::read(&second).unwrap();
    fs::write(destination_directory.join("merged (2).pdf"), b"occupied").unwrap();
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());
    let request = MergePdfRequest {
        source_paths: vec![first.clone(), second.clone(), first.clone()],
        destination_directory: destination_directory.clone(),
        output_name: "merged".to_owned(),
    };

    let first_result = merge_pdf(&engine, request.clone(), |_| {}).expect("merge should succeed");
    let second_result = merge_pdf(&engine, request, |_| {}).expect("repeat merge should succeed");

    assert_eq!(
        first_result.output_path,
        destination_directory.join("merged.pdf")
    );
    assert_eq!(
        second_result.output_path,
        destination_directory.join("merged (1).pdf")
    );
    assert_eq!(first_result.input_count, 3);
    assert_eq!(first_result.page_count, 5);
    assert!(!first_result.has_warnings);
    assert_eq!(
        inspect_document(&first_result.output_path)
            .unwrap()
            .page_count,
        5
    );
    let rendered_heights = [0, 1, 2, 4]
        .into_iter()
        .map(|page_index| {
            let mut rendered = Cursor::new(Vec::new());
            render_page_to_png(
                PdfRenderRequest {
                    source_path: first_result.output_path.clone(),
                    page_index,
                    target_width: 120,
                },
                &mut rendered,
            )
            .expect("representative merged page should render")
            .height_pixels
        })
        .collect::<Vec<_>>();
    assert_eq!(rendered_heights, [80, 180, 80, 180]);
    assert_eq!(fs::read(first).unwrap(), first_before);
    assert_eq!(fs::read(second).unwrap(), second_before);
    assert_eq!(
        fs::read(destination_directory.join("merged (2).pdf")).unwrap(),
        b"occupied"
    );
    assert_eq!(private_working_file_count(&destination_directory), 0);
}

#[test]
fn malformed_or_missing_merge_input_publishes_nothing() {
    install_pdfium_beside_test_executable();
    let directory = tempfile::tempdir().unwrap();
    let valid = fixture("one_page.pdf");
    let malformed = fixture("malformed.pdf");
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());

    let malformed_failure = merge_pdf(
        &engine,
        MergePdfRequest {
            source_paths: vec![valid.clone(), malformed.clone()],
            destination_directory: directory.path().to_path_buf(),
            output_name: "malformed-result.pdf".to_owned(),
        },
        |_| {},
    )
    .expect_err("malformed source should fail the whole merge");
    assert_eq!(
        malformed_failure.error.code,
        ApplicationErrorCode::InvalidPdf
    );
    assert_eq!(malformed_failure.input_index, Some(1));
    assert!(!directory.path().join("malformed-result.pdf").exists());

    let missing_failure = merge_pdf(
        &engine,
        MergePdfRequest {
            source_paths: vec![valid, directory.path().join("missing.pdf")],
            destination_directory: directory.path().to_path_buf(),
            output_name: "missing-result.pdf".to_owned(),
        },
        |_| {},
    )
    .expect_err("missing source should fail the whole merge");
    assert_eq!(
        missing_failure.error.code,
        ApplicationErrorCode::SourceNotFound
    );
    assert_eq!(missing_failure.input_index, Some(1));
    assert!(!directory.path().join("missing-result.pdf").exists());
    assert_eq!(private_working_file_count(directory.path()), 0);
}

#[test]
fn real_split_modes_preserve_ranges_geometry_unicode_paths_and_sources() {
    install_pdfium_beside_test_executable();
    let directory = tempfile::tempdir().unwrap();
    let source_directory = directory.path().join("แหล่ง PDF with spaces");
    fs::create_dir(&source_directory).unwrap();
    let source = source_directory.join("รายงาน ผสม.pdf");
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());
    engine
        .merge(&StructuralPdfMergeRequest {
            ordered_source_paths: vec![
                fixture("two_page.pdf"),
                fixture("one_page.pdf"),
                fixture("two_page.pdf"),
            ],
            working_output_path: source.clone(),
        })
        .unwrap();
    let source_before = fs::read(&source).unwrap();

    let cases = [
        (
            "ทุกหน้า",
            SplitPdfMode::EveryPage,
            vec![(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)],
        ),
        (
            "ทุกสองหน้า",
            SplitPdfMode::EveryNPages { pages_per_part: 2 },
            vec![(1, 2), (3, 4), (5, 5)],
        ),
        (
            "หลังหน้า",
            SplitPdfMode::SplitAfterPages {
                split_after_pages: "2, 4".to_owned(),
            },
            vec![(1, 2), (3, 4), (5, 5)],
        ),
    ];

    for (destination_name, mode, expected_ranges) in cases {
        let destination = directory.path().join(destination_name);
        fs::create_dir(&destination).unwrap();
        let result = split_pdf(
            &engine,
            SplitPdfRequest {
                source_path: source.clone(),
                destination_directory: destination.clone(),
                mode,
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(result.source_page_count, 5);
        assert_eq!(
            result
                .parts
                .iter()
                .map(|part| (part.range.first_page, part.range.last_page))
                .collect::<Vec<_>>(),
            expected_ranges
        );
        assert_eq!(
            result
                .parts
                .iter()
                .map(|part| inspect_document(&part.output_path).unwrap().page_count)
                .collect::<Vec<_>>(),
            result
                .parts
                .iter()
                .map(|part| part.range.page_count())
                .collect::<Vec<_>>()
        );
        assert_eq!(private_working_file_count(&destination), 0);
    }

    let split_after_directory = directory.path().join("หลังหน้า").join("รายงาน ผสม-split");
    let rendered_heights = [
        (split_after_directory.join("รายงาน ผสม-part-0001.pdf"), 0),
        (split_after_directory.join("รายงาน ผสม-part-0001.pdf"), 1),
        (split_after_directory.join("รายงาน ผสม-part-0002.pdf"), 0),
        (split_after_directory.join("รายงาน ผสม-part-0002.pdf"), 1),
        (split_after_directory.join("รายงาน ผสม-part-0003.pdf"), 0),
    ]
    .into_iter()
    .map(|(path, page_index)| {
        let mut rendered = Cursor::new(Vec::new());
        render_page_to_png(
            PdfRenderRequest {
                source_path: path,
                page_index,
                target_width: 120,
            },
            &mut rendered,
        )
        .unwrap()
        .height_pixels
    })
    .collect::<Vec<_>>();
    assert_eq!(rendered_heights, [80, 180, 80, 80, 180]);
    assert_eq!(fs::read(source).unwrap(), source_before);
}

#[test]
fn real_size_split_measures_outputs_and_impossible_limit_publishes_nothing() {
    install_pdfium_beside_test_executable();
    let directory = tempfile::tempdir().unwrap();
    let engine = QpdfCliEngine::from_runtime_root(qpdf_runtime_root());
    let source = directory.path().join("large pages.pdf");
    create_large_content_pdf(&source, 3, 700_000);
    let source_before = fs::read(&source).unwrap();

    let result = split_pdf(
        &engine,
        SplitPdfRequest {
            source_path: source.clone(),
            destination_directory: directory.path().to_path_buf(),
            mode: SplitPdfMode::MaximumFileSize { maximum_size_mb: 1 },
        },
        |_| {},
    )
    .expect("controlled large pages should split at actual 1 MB boundaries");
    assert!(result.parts.len() >= 2);
    assert!(result.parts.iter().all(|part| part.size_bytes <= 1_000_000));
    assert_eq!(
        result
            .parts
            .iter()
            .flat_map(|part| part.range.first_page..=part.range.last_page)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(fs::read(&source).unwrap(), source_before);

    let impossible_source = directory.path().join("impossible.pdf");
    create_large_content_pdf(&impossible_source, 2, 2_400_000);
    let impossible_before = fs::read(&impossible_source).unwrap();
    let failure = split_pdf(
        &engine,
        SplitPdfRequest {
            source_path: impossible_source.clone(),
            destination_directory: directory.path().to_path_buf(),
            mode: SplitPdfMode::MaximumFileSize { maximum_size_mb: 1 },
        },
        |_| {},
    )
    .expect_err("a page over the hard limit cannot be structurally reduced");
    assert_eq!(
        failure.error.code,
        ApplicationErrorCode::SplitPageExceedsSizeLimit
    );
    assert_eq!(failure.page_number, Some(1));
    assert!(failure.actual_size_bytes.unwrap() > 1_000_000);
    assert!(!directory.path().join("impossible-split").exists());
    assert_eq!(fs::read(impossible_source).unwrap(), impossible_before);
    assert_eq!(private_working_file_count(directory.path()), 0);
}

fn install_pdfium_beside_test_executable() {
    let executable = std::env::current_exe().expect("test executable should resolve");
    let destination = executable
        .parent()
        .expect("test executable should have a parent")
        .join("pdfium.dll");
    if !destination.is_file() {
        fs::copy(pdfium_library(), destination)
            .expect("PDFium should be copied beside the integration-test executable");
    }
}

fn private_working_file_count(directory: &Path) -> usize {
    fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".ilikepdf-")
        })
        .count()
}

fn create_large_content_pdf(path: &Path, page_count: usize, content_bytes: usize) {
    let object_count = 2 + page_count * 2;
    let page_object = |index: usize| 3 + index * 2;
    let content_object = |index: usize| 4 + index * 2;
    let mut output = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![0_usize; object_count + 1];
    let mut append_object = |number: usize, body: &[u8]| {
        offsets[number] = output.len();
        writeln!(output, "{number} 0 obj").unwrap();
        output.extend_from_slice(body);
        output.extend_from_slice(b"\nendobj\n");
    };
    append_object(1, b"<< /Type /Catalog /Pages 2 0 R >>");
    let mut pages = String::from("<< /Type /Pages /Kids [");
    for index in 0..page_count {
        write!(pages, " {} 0 R", page_object(index)).unwrap();
    }
    write!(pages, " ] /Count {page_count} >>").unwrap();
    append_object(2, pages.as_bytes());
    for index in 0..page_count {
        let page = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << >> /Contents {} 0 R >>",
            content_object(index)
        );
        append_object(page_object(index), page.as_bytes());
        let mut content = Vec::with_capacity(content_bytes);
        let mut state = 0x9e37_79b9_u32.wrapping_add(index as u32);
        while content.len() + 11 <= content_bytes {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            writeln!(content, "%{state:08x}").unwrap();
        }
        content.resize(content_bytes, b' ');
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend_from_slice(&content);
        stream.extend_from_slice(b"\nendstream");
        append_object(content_object(index), &stream);
    }
    let xref_offset = output.len();
    write!(output, "xref\n0 {}\n", object_count + 1).unwrap();
    output.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        writeln!(output, "{offset:010} 00000 n ").unwrap();
    }
    write!(
        output,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
        object_count + 1
    )
    .unwrap();
    fs::write(path, output).unwrap();
}
