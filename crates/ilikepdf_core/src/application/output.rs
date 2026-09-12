//! Safe destination validation, naming, and atomic publication for generated files.

mod image;
mod pdf;
mod publication;

pub(crate) use image::{
    PendingImageOutput, validate_output_directory, validate_writable_output_directory,
};
pub(crate) use pdf::PendingPdfOutput;
pub(crate) use publication::{collision_key, occupied_names};
