//! Flutter FFI contracts, mapping, and native application entry points.
//!
//! This crate is the composition boundary: it converts Dart-friendly values to
//! stable core requests and keeps generated bridge code separate from hand-written APIs.

pub mod api;
mod frb_generated;
mod logging;
