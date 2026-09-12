use std::path::PathBuf;

use super::*;

#[test]
fn rejects_unsafe_render_widths_before_creating_output() {
    let error = render_pdf_page(RenderPdfPageRequest {
        source_path: PathBuf::from("not-opened.pdf"),
        page_index: 0,
        target_width: 63,
        destination_path: None,
    })
    .expect_err("small render widths should be rejected");

    assert_eq!(error.code, ApplicationErrorCode::InvalidRequest);
}
