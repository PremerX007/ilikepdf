use std::ffi::{OsStr, OsString};
use std::path::Path;

use ilikepdf_pdf::{
    ImagePdfLayout as NativeLayout, ImagePdfMargin as NativeMargin,
    ImagePdfOrientation as NativeOrientation, ImagePdfPageSize as NativePageSize,
};

use super::{CreateImagePdfRequest, ImagePdfMargin, ImagePdfOrientation, ImagePdfPageSize};
use crate::{ApplicationError, ApplicationErrorCode, ApplicationResult};

pub(super) fn native_layout(request: &CreateImagePdfRequest) -> NativeLayout {
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

pub(super) fn output_stems(request: &CreateImagePdfRequest) -> ApplicationResult<Vec<OsString>> {
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

pub(super) fn invalid_request(message: &'static str) -> ApplicationError {
    ApplicationError::new(ApplicationErrorCode::InvalidRequest, message)
}
