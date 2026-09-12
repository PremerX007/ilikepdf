use std::ffi::OsString;
use std::path::PathBuf;

use ilikepdf_pdf::ImagePdfLayout as NativeLayout;

use super::backend::{ImagePdfBackend, NativeImagePdfBackend};
use super::planning::{invalid_request, native_layout, output_stems};
use super::{CreateImagePdfRequest, ImagePdfFailure, ImagePdfProgress, ImagePdfResult};
use crate::ApplicationError;
use crate::application::output::{PendingPdfOutput, validate_output_directory};

pub fn create_pdfs_from_images(
    request: CreateImagePdfRequest,
    on_progress: impl FnMut(ImagePdfProgress),
) -> Result<ImagePdfResult, ImagePdfFailure> {
    create_pdfs_from_images_with_backend(&NativeImagePdfBackend, request, on_progress)
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
mod tests;
