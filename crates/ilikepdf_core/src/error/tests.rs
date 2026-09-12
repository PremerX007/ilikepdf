use super::*;

#[test]
fn structured_error_has_a_safe_display_message() {
    let error = ApplicationError {
        code: ApplicationErrorCode::Internal,
        message: "Unable to complete the operation".to_owned(),
    };

    assert_eq!(error.to_string(), "Unable to complete the operation");
}
