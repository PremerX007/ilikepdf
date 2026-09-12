use std::path::{Path, PathBuf};

use ilikepdf_pdf::{ImagePdfLayout as NativeLayout, ImagePdfRequest as NativeRequest};

use crate::ApplicationResult;

pub(super) trait ImagePdfBackend {
    fn validate_image(&self, source_path: &Path) -> ApplicationResult<()>;

    fn create_pdf(
        &self,
        source_paths: Vec<PathBuf>,
        layout: NativeLayout,
        output: &mut std::fs::File,
        on_page_complete: &mut dyn FnMut(),
    ) -> ApplicationResult<()>;
}

pub(super) struct NativeImagePdfBackend;

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
