use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use ilikepdf_pdf::{
    ImagePdfLayout as NativeLayout, ImagePdfMargin as NativeMargin,
    ImagePdfOrientation as NativeOrientation, ImagePdfPageSize as NativePageSize,
    ImagePdfRequest as NativeRequest,
};

use super::pdf_output::PendingPdfOutput;
use super::png_output::validate_output_directory;
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePdfPageSize {
    Fit,
    A4,
    UsLetter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePdfOrientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePdfMargin {
    None,
    Small,
    Big,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateImagePdfRequest {
    pub source_paths: Vec<PathBuf>,
    pub destination_directory: PathBuf,
    pub page_size: ImagePdfPageSize,
    pub orientation: ImagePdfOrientation,
    pub margin: ImagePdfMargin,
    pub merge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePdfProgress {
    pub total_image_count: u32,
    pub completed_image_count: u32,
    pub current_image: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePdfResult {
    pub total_image_count: u32,
    pub completed_image_count: u32,
    pub output_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePdfFailure {
    pub total_image_count: u32,
    pub completed_image_count: u32,
    pub current_image: Option<u32>,
    pub output_files: Vec<PathBuf>,
    pub error: ApplicationError,
}

pub fn create_pdfs_from_images(
    request: CreateImagePdfRequest,
    on_progress: impl FnMut(ImagePdfProgress),
) -> Result<ImagePdfResult, ImagePdfFailure> {
    create_pdfs_from_images_with_backend(&NativeImagePdfBackend, request, on_progress)
}

trait ImagePdfBackend {
    fn validate_image(&self, source_path: &Path) -> ApplicationResult<()>;

    fn create_pdf(
        &self,
        source_paths: Vec<PathBuf>,
        layout: NativeLayout,
        output: &mut std::fs::File,
        on_page_complete: &mut dyn FnMut(),
    ) -> ApplicationResult<()>;
}

struct NativeImagePdfBackend;

impl ImagePdfBackend for NativeImagePdfBackend {
    fn validate_image(&self, source_path: &Path) -> ApplicationResult<()> {
        ilikepdf_pdf::inspect_image(source_path)
            .map(|_| ())
            .map_err(Into::into)
    }

    fn create_pdf(
        &self,
        source_paths: Vec<PathBuf>,
        layout: NativeLayout,
        output: &mut std::fs::File,
        on_page_complete: &mut dyn FnMut(),
    ) -> ApplicationResult<()> {
        ilikepdf_pdf::create_image_pdf(
            NativeRequest {
                source_paths,
                layout,
            },
            output,
            |_| on_page_complete(),
        )
        .map(|_| ())
        .map_err(Into::into)
    }
}

impl ImagePdfBackend for ilikepdf_pdf::PdfRenderer {
    fn validate_image(&self, source_path: &Path) -> ApplicationResult<()> {
        self.inspect_image(source_path)
            .map(|_| ())
            .map_err(Into::into)
    }

    fn create_pdf(
        &self,
        source_paths: Vec<PathBuf>,
        layout: NativeLayout,
        output: &mut std::fs::File,
        on_page_complete: &mut dyn FnMut(),
    ) -> ApplicationResult<()> {
        self.create_image_pdf(
            NativeRequest {
                source_paths,
                layout,
            },
            output,
            |_| on_page_complete(),
        )
        .map(|_| ())
        .map_err(Into::into)
    }
}

fn create_pdfs_from_images_with_backend(
    backend: &impl ImagePdfBackend,
    request: CreateImagePdfRequest,
    mut on_progress: impl FnMut(ImagePdfProgress),
) -> Result<ImagePdfResult, ImagePdfFailure> {
    let total_image_count = u32::try_from(request.source_paths.len()).map_err(|_| {
        ImagePdfFailure::before_start(invalid_request("Too many images were selected"))
    })?;
    if total_image_count == 0 {
        return Err(ImagePdfFailure::before_start(invalid_request(
            "Select at least one image",
        )));
    }
    validate_output_directory(&request.destination_directory)
        .map_err(|error| ImagePdfFailure::with_total(total_image_count, error))?;

    let output_stems = output_stems(&request)
        .map_err(|error| ImagePdfFailure::with_total(total_image_count, error))?;

    let layout = native_layout(&request);
    if request.merge {
        return create_merged_pdf(
            backend,
            request.source_paths,
            request.destination_directory,
            output_stems
                .into_iter()
                .next()
                .expect("merged output plan contains one stem"),
            layout,
            total_image_count,
            on_progress,
        );
    }

    for (index, source_path) in request.source_paths.iter().enumerate() {
        backend.validate_image(source_path).map_err(|error| {
            ImagePdfFailure::at_image(total_image_count, index, Vec::new(), error)
        })?;
    }

    on_progress(ImagePdfProgress {
        total_image_count,
        completed_image_count: 0,
        current_image: Some(1),
    });
    let mut output_files = Vec::with_capacity(output_stems.len());
    for (index, (source_path, output_stem)) in request
        .source_paths
        .into_iter()
        .zip(output_stems)
        .enumerate()
    {
        let mut pending =
            PendingPdfOutput::in_directory(&request.destination_directory).map_err(|error| {
                ImagePdfFailure::at_image(total_image_count, index, output_files.clone(), error)
            })?;
        backend
            .create_pdf(
                vec![source_path],
                layout,
                pending.file.as_file_mut(),
                &mut || {},
            )
            .map_err(|error| {
                ImagePdfFailure::at_image(total_image_count, index, output_files.clone(), error)
            })?;
        let published = pending.publish_with_stem(&output_stem).map_err(|error| {
            ImagePdfFailure::at_image(total_image_count, index, output_files.clone(), error)
        })?;
        output_files.push(published);
        let completed = u32::try_from(index + 1).expect("image count was represented by u32");
        on_progress(ImagePdfProgress {
            total_image_count,
            completed_image_count: completed,
            current_image: (completed < total_image_count).then_some(completed + 1),
        });
    }

    Ok(ImagePdfResult {
        total_image_count,
        completed_image_count: total_image_count,
        output_files,
    })
}

fn create_merged_pdf(
    backend: &impl ImagePdfBackend,
    source_paths: Vec<PathBuf>,
    destination_directory: PathBuf,
    output_stem: OsString,
    layout: NativeLayout,
    total_image_count: u32,
    mut on_progress: impl FnMut(ImagePdfProgress),
) -> Result<ImagePdfResult, ImagePdfFailure> {
    on_progress(ImagePdfProgress {
        total_image_count,
        completed_image_count: 0,
        current_image: Some(1),
    });
    let mut pending = PendingPdfOutput::in_directory(&destination_directory)
        .map_err(|error| ImagePdfFailure::with_total(total_image_count, error))?;
    let mut completed_image_count = 0;
    backend
        .create_pdf(
            source_paths,
            layout,
            pending.file.as_file_mut(),
            &mut || {
                completed_image_count += 1;
                on_progress(ImagePdfProgress {
                    total_image_count,
                    completed_image_count,
                    current_image: (completed_image_count < total_image_count)
                        .then_some(completed_image_count + 1),
                });
            },
        )
        .map_err(|error| ImagePdfFailure {
            total_image_count,
            completed_image_count,
            current_image: (completed_image_count < total_image_count)
                .then_some(completed_image_count + 1),
            output_files: Vec::new(),
            error,
        })?;
    let published = pending
        .publish_with_stem(&output_stem)
        .map_err(|error| ImagePdfFailure {
            total_image_count,
            completed_image_count,
            current_image: None,
            output_files: Vec::new(),
            error,
        })?;

    Ok(ImagePdfResult {
        total_image_count,
        completed_image_count,
        output_files: vec![published],
    })
}

fn native_layout(request: &CreateImagePdfRequest) -> NativeLayout {
    NativeLayout {
        page_size: match request.page_size {
            ImagePdfPageSize::Fit => NativePageSize::Fit,
            ImagePdfPageSize::A4 => NativePageSize::A4,
            ImagePdfPageSize::UsLetter => NativePageSize::UsLetter,
        },
        orientation: match request.orientation {
            ImagePdfOrientation::Portrait => NativeOrientation::Portrait,
            ImagePdfOrientation::Landscape => NativeOrientation::Landscape,
        },
        margin: match request.margin {
            ImagePdfMargin::None => NativeMargin::None,
            ImagePdfMargin::Small => NativeMargin::Small,
            ImagePdfMargin::Big => NativeMargin::Big,
        },
    }
}

fn output_stems(request: &CreateImagePdfRequest) -> ApplicationResult<Vec<OsString>> {
    if request.merge {
        return Ok(vec![source_stem(&request.source_paths[0])?.to_os_string()]);
    }

    request
        .source_paths
        .iter()
        .map(|source_path| source_stem(source_path).map(OsStr::to_os_string))
        .collect()
}

fn source_stem(source_path: &Path) -> ApplicationResult<&OsStr> {
    source_path
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| invalid_request("Each image must have a file name"))
}

fn invalid_request(message: &'static str) -> ApplicationError {
    ApplicationError::new(ApplicationErrorCode::InvalidRequest, message)
}

impl ImagePdfFailure {
    fn before_start(error: ApplicationError) -> Self {
        Self {
            total_image_count: 0,
            completed_image_count: 0,
            current_image: None,
            output_files: Vec::new(),
            error,
        }
    }

    fn with_total(total_image_count: u32, error: ApplicationError) -> Self {
        Self {
            total_image_count,
            completed_image_count: 0,
            current_image: None,
            output_files: Vec::new(),
            error,
        }
    }

    fn at_image(
        total_image_count: u32,
        zero_based_index: usize,
        output_files: Vec<PathBuf>,
        error: ApplicationError,
    ) -> Self {
        Self {
            total_image_count,
            completed_image_count: u32::try_from(output_files.len())
                .expect("published outputs cannot exceed selected images"),
            current_image: Some(
                u32::try_from(zero_based_index + 1).expect("image count was represented by u32"),
            ),
            output_files,
            error,
        }
    }
}

#[cfg(test)]
mod tests {
    use ilikepdf_pdf::PdfRenderer;
    use std::fs;

    use super::*;

    fn request(paths: Vec<PathBuf>, merge: bool) -> CreateImagePdfRequest {
        CreateImagePdfRequest {
            source_paths: paths,
            destination_directory: PathBuf::from("output"),
            page_size: ImagePdfPageSize::A4,
            orientation: ImagePdfOrientation::Portrait,
            margin: ImagePdfMargin::None,
            merge,
        }
    }

    #[test]
    fn naming_uses_the_first_image_in_final_order() {
        let single = output_stems(&request(vec![PathBuf::from("passport.jpg")], true))
            .expect("single output stem");
        let multiple = output_stems(&request(
            vec![PathBuf::from("01.jpg"), PathBuf::from("02.png")],
            true,
        ))
        .expect("merged output stem");
        let reordered = output_stems(&request(
            vec![PathBuf::from("02.png"), PathBuf::from("01.jpg")],
            true,
        ))
        .expect("reordered output stem");
        let separate = output_stems(&request(
            vec![PathBuf::from("01.jpg"), PathBuf::from("02.png")],
            false,
        ))
        .expect("separate output stems");

        assert_eq!(single, [OsString::from("passport")]);
        assert_eq!(multiple, [OsString::from("01")]);
        assert_eq!(reordered, [OsString::from("02")]);
        assert_eq!(separate, [OsString::from("01"), OsString::from("02")]);
    }

    #[test]
    fn duplicate_unmerged_stems_are_kept_in_conversion_order() {
        let stems = output_stems(&request(
            vec![PathBuf::from("A/scan.jpg"), PathBuf::from("B/SCAN.png")],
            false,
        ))
        .expect("duplicate stems are numbered during publication");

        assert_eq!(stems, [OsString::from("scan"), OsString::from("SCAN")]);
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("ilikepdf_pdf")
            .join("tests")
            .join("fixtures")
            .join("images")
            .join(name)
    }

    fn renderer() -> &'static PdfRenderer {
        crate::test_pdf_renderer()
    }

    fn real_request(
        source_paths: Vec<PathBuf>,
        destination_directory: PathBuf,
        merge: bool,
    ) -> CreateImagePdfRequest {
        CreateImagePdfRequest {
            source_paths,
            destination_directory,
            page_size: ImagePdfPageSize::A4,
            orientation: ImagePdfOrientation::Portrait,
            margin: ImagePdfMargin::None,
            merge,
        }
    }

    #[test]
    fn merged_and_unmerged_outputs_publish_with_expected_names_and_page_counts() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let merged_destination = directory.path().join("merged");
        let separate_destination = directory.path().join("separate");
        fs::create_dir(&merged_destination).expect("merged directory should be created");
        fs::create_dir(&separate_destination).expect("separate directory should be created");
        let sources = vec![
            fixture("portrait.png"),
            fixture("photo.jpg"),
            fixture("sample.webp"),
        ];
        let source_bytes = sources
            .iter()
            .map(|path| fs::read(path).expect("fixture should be readable"))
            .collect::<Vec<_>>();

        let mut merged_progress = Vec::new();
        let merged = create_pdfs_from_images_with_backend(
            renderer(),
            real_request(sources.clone(), merged_destination.clone(), true),
            |progress| merged_progress.push(progress),
        )
        .expect("merged PDF should succeed");
        assert_eq!(
            merged.output_files,
            [merged_destination.join("portrait.pdf")]
        );
        assert_eq!(
            renderer()
                .inspect_document(&merged.output_files[0])
                .expect("merged PDF should reopen")
                .page_count,
            3
        );
        assert_eq!(
            merged_progress
                .last()
                .expect("progress should be reported")
                .completed_image_count,
            3
        );

        let separate = create_pdfs_from_images_with_backend(
            renderer(),
            real_request(sources.clone(), separate_destination.clone(), false),
            |_| {},
        )
        .expect("separate PDFs should succeed");
        assert_eq!(
            separate.output_files,
            [
                separate_destination.join("portrait.pdf"),
                separate_destination.join("photo.pdf"),
                separate_destination.join("sample.pdf"),
            ]
        );
        for output in &separate.output_files {
            assert_eq!(
                renderer()
                    .inspect_document(output)
                    .expect("separate PDF should reopen")
                    .page_count,
                1
            );
        }

        for (source, expected) in sources.iter().zip(source_bytes) {
            assert_eq!(
                fs::read(source).expect("source should remain readable"),
                expected
            );
        }
    }

    #[test]
    fn merged_collisions_use_the_lowest_gap_and_never_clobber() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let base = directory.path().join("portrait.pdf");
        let second = directory.path().join("portrait (2).pdf");
        fs::write(&base, b"existing base bytes").expect("base fixture should be written");
        fs::write(&second, b"existing second bytes").expect("numbered fixture should be written");

        let result = create_pdfs_from_images_with_backend(
            renderer(),
            real_request(
                vec![fixture("portrait.png")],
                directory.path().to_path_buf(),
                true,
            ),
            |_| {},
        )
        .expect("ordinary collisions should be numbered");

        assert_eq!(
            result.output_files,
            [directory.path().join("portrait (1).pdf")]
        );
        assert_eq!(
            fs::read(base).expect("base output should remain readable"),
            b"existing base bytes"
        );
        assert_eq!(
            fs::read(second).expect("numbered output should remain readable"),
            b"existing second bytes"
        );
    }

    #[test]
    fn unmerged_collisions_and_duplicate_stems_are_numbered_in_order() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let source_one = directory.path().join("one");
        let source_two = directory.path().join("two");
        let destination = directory.path().join("output");
        fs::create_dir(&source_one).expect("first source directory should be created");
        fs::create_dir(&source_two).expect("second source directory should be created");
        fs::create_dir(&destination).expect("output directory should be created");
        let first_scan = source_one.join("scan.jpg");
        let second_scan = source_two.join("scan.png");
        fs::copy(fixture("photo.jpg"), &first_scan).expect("JPEG fixture should be copied");
        fs::copy(fixture("portrait.png"), &second_scan).expect("PNG fixture should be copied");
        let existing = destination.join("scan.pdf");
        fs::write(&existing, b"existing PDF bytes").expect("existing output should be written");

        let result = create_pdfs_from_images_with_backend(
            renderer(),
            real_request(vec![first_scan, second_scan], destination.clone(), false),
            |_| {},
        )
        .expect("duplicate stems should receive distinct output names");

        assert_eq!(
            result.output_files,
            [
                destination.join("scan (1).pdf"),
                destination.join("scan (2).pdf")
            ]
        );
        assert_eq!(
            fs::read(existing).expect("existing output should remain readable"),
            b"existing PDF bytes"
        );
    }

    #[test]
    fn unmerged_validation_fails_before_any_output_is_published() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let failure = create_pdfs_from_images_with_backend(
            renderer(),
            real_request(
                vec![fixture("portrait.png"), fixture("malformed.png")],
                directory.path().to_path_buf(),
                false,
            ),
            |_| {},
        )
        .expect_err("malformed input should fail preflight");

        assert!(matches!(
            failure.error.code,
            ApplicationErrorCode::MalformedImage | ApplicationErrorCode::ImageDecodeFailed
        ));
        assert!(failure.output_files.is_empty());
        assert!(!directory.path().join("portrait.pdf").exists());
    }
}
