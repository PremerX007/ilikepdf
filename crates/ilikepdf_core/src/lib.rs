#![forbid(unsafe_code)]

pub mod application;
pub mod error;

pub use application::app_info::{ApplicationInfo, get_application_info};
pub use error::{ApplicationError, ApplicationErrorCode, ApplicationResult};
