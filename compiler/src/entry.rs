//! Executable entry-point configuration shared across compiler stages.

use crate::lexer::is_valid_identifier_name;

pub(crate) const DEFAULT_ENTRY_NAME: &str = "main";

pub(crate) fn validate_entry_name(name: &str) -> Result<(), String> {
    if is_valid_identifier_name(name) {
        Ok(())
    } else {
        Err(format!(
            "entry name `{name}` is not a valid Nocter identifier"
        ))
    }
}
