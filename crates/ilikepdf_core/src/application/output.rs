//! Safe destination validation, naming, and atomic publication for generated files.

mod image;
mod pdf;
mod publication;
mod split;

pub(crate) use image::{
    PendingImageOutput, validate_output_directory, validate_writable_output_directory,
};
pub(crate) use pdf::{PendingExactPdfOutput, PendingNumberedPdfOutput, PendingPdfOutput};
pub(crate) use publication::{collision_key, occupied_names};
pub(crate) use split::PendingSplitDirectory;
