use std::path::PathBuf;

use ilikepdf_pdf::PdfDpiRenderRequest;

use crate::ApplicationError;
use crate::application::output::{PendingImageOutput, validate_output_directory};

use super::model::{
    ExportPdfToImagesRequest, PdfExportFailure, PdfExportProgress, PdfExportResult,
};
use super::plan::output_plan;

pub fn export_pdf_to_images(
    request: ExportPdfToImagesRequest,
    on_progress: impl FnMut(PdfExportProgress),
) -> Result<PdfExportResult, PdfExportFailure> {
    export_pdf_to_images_with_backend(&NativePdfBackend, request, on_progress)
}

use super::backend::{NativePdfBackend, PdfBackend};

pub(crate) fn export_pdf_to_images_with_backend(
    backend: &impl PdfBackend,
    request: ExportPdfToImagesRequest,
    on_progress: impl FnMut(PdfExportProgress),
) -> Result<PdfExportResult, PdfExportFailure> {
    let info = backend
        .inspect_document(&request.source_path)
        .map_err(PdfExportFailure::before_start)?;
    export_inspected_pdf_to_images_with_backend(backend, request, info.page_count, on_progress)
}

pub(crate) fn export_inspected_pdf_to_images_with_backend(
    backend: &impl PdfBackend,
    request: ExportPdfToImagesRequest,
    total_page_count: u32,
    mut on_progress: impl FnMut(PdfExportProgress),
) -> Result<PdfExportResult, PdfExportFailure> {
    validate_output_directory(&request.destination_directory)
        .map_err(|error| PdfExportFailure::with_total(total_page_count, error))?;

    let output_plan = output_plan(
        &request.source_path,
        &request.destination_directory,
        total_page_count,
        request.format,
    )
    .map_err(|error| PdfExportFailure::with_total(total_page_count, error))?;
    let destinations = output_plan.destinations;

    let mut output_files = Vec::with_capacity(destinations.len());
    if total_page_count == 0 {
        return Ok(PdfExportResult {
            total_page_count,
            completed_page_count: 0,
            output_files,
        });
    }

    on_progress(PdfExportProgress {
        total_page_count,
        completed_page_count: 0,
        current_page: Some(1),
    });

    for (page_index, destination) in destinations.into_iter().enumerate() {
        let current_page = u32::try_from(page_index + 1).expect("page count is represented by u32");
        let mut output = if output_plan.number_single_file {
            PendingImageOutput::for_numbered_destination(&destination, request.format.extension())
        } else {
            PendingImageOutput::for_destination(&destination, request.format.extension())
        }
        .map_err(|error| {
            PdfExportFailure::during_export(
                total_page_count,
                current_page,
                output_files.clone(),
                error,
            )
        })?;
        backend
            .render_page_to_image_at_dpi(
                PdfDpiRenderRequest {
                    source_path: request.source_path.clone(),
                    page_index: current_page - 1,
                    dpi: request.quality.dpi(),
                },
                request.format.renderer_format(),
                output.file.as_file_mut(),
            )
            .map_err(|error| {
                PdfExportFailure::during_export(
                    total_page_count,
                    current_page,
                    output_files.clone(),
                    error,
                )
            })?;
        let published = output.publish().map_err(|error| {
            PdfExportFailure::during_export(
                total_page_count,
                current_page,
                output_files.clone(),
                error,
            )
        })?;
        output_files.push(published);

        on_progress(PdfExportProgress {
            total_page_count,
            completed_page_count: current_page,
            current_page: Some(current_page.saturating_add(1).min(total_page_count)),
        });
    }

    Ok(PdfExportResult {
        total_page_count,
        completed_page_count: total_page_count,
        output_files,
    })
}

impl PdfExportFailure {
    fn before_start(error: ApplicationError) -> Self {
        Self {
            total_page_count: 0,
            completed_page_count: 0,
            current_page: None,
            output_files: Vec::new(),
            error,
        }
    }

    fn with_total(total_page_count: u32, error: ApplicationError) -> Self {
        Self {
            total_page_count,
            completed_page_count: 0,
            current_page: None,
            output_files: Vec::new(),
            error,
        }
    }

    fn during_export(
        total_page_count: u32,
        current_page: u32,
        output_files: Vec<PathBuf>,
        error: ApplicationError,
    ) -> Self {
        Self {
            total_page_count,
            completed_page_count: u32::try_from(output_files.len())
                .expect("output count cannot exceed the PDF page count"),
            current_page: Some(current_page),
            output_files,
            error,
        }
    }
}

#[cfg(test)]
mod tests;
