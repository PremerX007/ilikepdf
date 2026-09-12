//! Image decoding and PDF document creation backed by PDFium.

use std::io::Write;
use std::path::PathBuf;

use pdfium_render::prelude::{
    PdfPageImageObject, PdfPageObjectsCommon, PdfPagePaperSize, PdfPoints, Pdfium,
};

use crate::{PdfError, PdfErrorKind};

mod image_input;
mod layout;

use image_input::decode_visually_oriented_image;
pub(crate) use image_input::inspect_image;
use layout::calculate_page_layout;

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

impl ImagePdfMargin {
    const fn millimeters(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Small => 10.0,
            Self::Big => 20.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImagePdfLayout {
    pub page_size: ImagePdfPageSize,
    pub orientation: ImagePdfOrientation,
    pub margin: ImagePdfMargin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePdfRequest {
    pub source_paths: Vec<PathBuf>,
    pub layout: ImagePdfLayout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImagePdfPageInfo {
    pub width_points: f32,
    pub height_points: f32,
    pub image_left_points: f32,
    pub image_bottom_points: f32,
    pub image_width_points: f32,
    pub image_height_points: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImagePdfResult {
    pub page_count: u32,
    pub pages: Vec<ImagePdfPageInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub width_pixels: u32,
    pub height_pixels: u32,
}

pub(crate) fn create_image_pdf<W: Write + 'static>(
    pdfium: &Pdfium,
    request: ImagePdfRequest,
    output: &mut W,
    mut on_page_complete: impl FnMut(ImagePdfPageInfo),
) -> Result<ImagePdfResult, PdfError> {
    if request.source_paths.is_empty() {
        return Err(PdfError::new(PdfErrorKind::DocumentCreateFailed));
    }

    let mut document = pdfium
        .create_new_pdf()
        .map_err(|_| PdfError::new(PdfErrorKind::DocumentCreateFailed))?;
    let mut page_infos = Vec::with_capacity(request.source_paths.len());

    for source_path in &request.source_paths {
        let decoded = decode_visually_oriented_image(source_path)?;
        let page_info = calculate_page_layout(
            decoded.image.width(),
            decoded.image.height(),
            request.layout,
        )?;
        let page_size = PdfPagePaperSize::new_custom(
            PdfPoints::new(page_info.width_points),
            PdfPoints::new(page_info.height_points),
        );
        let mut page = document
            .pages_mut()
            .create_page_at_end(page_size)
            .map_err(|_| PdfError::new(PdfErrorKind::PageCreateFailed))?;
        if decoded.can_embed_source_jpeg {
            let mut image_object =
                PdfPageImageObject::new_from_jpeg_file(&document, source_path)
                    .map_err(|_| PdfError::new(PdfErrorKind::ImagePlacementFailed))?;
            image_object
                .scale(page_info.image_width_points, page_info.image_height_points)
                .and_then(|_| {
                    image_object.translate(
                        PdfPoints::new(page_info.image_left_points),
                        PdfPoints::new(page_info.image_bottom_points),
                    )
                })
                .map_err(|_| PdfError::new(PdfErrorKind::ImagePlacementFailed))?;
            page.objects_mut()
                .add_image_object(image_object)
                .map_err(|_| PdfError::new(PdfErrorKind::ImagePlacementFailed))?;
        } else {
            page.objects_mut()
                .create_image_object(
                    PdfPoints::new(page_info.image_left_points),
                    PdfPoints::new(page_info.image_bottom_points),
                    &decoded.image,
                    Some(PdfPoints::new(page_info.image_width_points)),
                    Some(PdfPoints::new(page_info.image_height_points)),
                )
                .map_err(|_| PdfError::new(PdfErrorKind::ImagePlacementFailed))?;
        }
        drop(page);
        drop(decoded);

        on_page_complete(page_info.clone());
        page_infos.push(page_info);
    }

    document
        .save_to_writer(output)
        .map_err(|_| PdfError::new(PdfErrorKind::SaveFailed))?;

    Ok(ImagePdfResult {
        page_count: u32::try_from(page_infos.len())
            .map_err(|_| PdfError::new(PdfErrorKind::DocumentCreateFailed))?,
        pages: page_infos,
    })
}
