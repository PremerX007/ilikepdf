use std::sync::Once;

use log::{LevelFilter, Log, Metadata, Record};

static LOGGER: PrivacySafeLogger = PrivacySafeLogger;
static INITIALIZE: Once = Once::new();

struct PrivacySafeLogger;

impl Log for PrivacySafeLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target().starts_with("ilikepdf_")
            && metadata.level()
                <= if cfg!(debug_assertions) {
                    LevelFilter::Info
                } else {
                    LevelFilter::Warn
                }
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!(
                "[ilikepdf][{}][{}] {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Event {
    ApplicationInfoRequested,
    CoreInitialized,
    PdfDocumentOpenRequested,
    PdfImageExportRequested,
    ImagePdfCreationRequested,
    PdfPageRenderRequested,
}

impl Event {
    const fn name(self) -> &'static str {
        match self {
            Self::ApplicationInfoRequested => "application_info_requested",
            Self::CoreInitialized => "core_initialized",
            Self::PdfDocumentOpenRequested => "pdf_document_open_requested",
            Self::PdfImageExportRequested => "pdf_image_export_requested",
            Self::ImagePdfCreationRequested => "image_pdf_creation_requested",
            Self::PdfPageRenderRequested => "pdf_page_render_requested",
        }
    }
}

pub(crate) fn init() {
    INITIALIZE.call_once(|| {
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(LevelFilter::Info);
        }
    });
}

/// Records only allow-listed, static events. Never add document paths, passwords,
/// document metadata, or user-provided content to logging fields.
pub(crate) fn record(event: Event) {
    log::info!(target: "ilikepdf_bridge", "event={}", event.name());
}
