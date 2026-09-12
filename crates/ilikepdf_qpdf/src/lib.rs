#![forbid(unsafe_code)]
//! qpdf command-line infrastructure for engine-neutral structural PDF workflows.
//!
//! This crate owns bundled-runtime resolution, direct process invocation, bounded
//! diagnostics, and qpdf command semantics. Application code consumes only the
//! [`ilikepdf_core::StructuralPdfEngine`] contract.

mod engine;
mod manifest;
mod process;
mod runtime;

pub use engine::QpdfCliEngine;
