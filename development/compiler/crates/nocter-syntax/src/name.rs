use crate::{ContextualSpelling, Keyword};

/// Reports whether `text` is one complete source-level name.
///
/// This is the shared boundary used by the parser and tooling mutations. Contextual spellings
/// remain valid names; reserved keywords, `_`, and `Self` do not.
#[must_use]
pub fn is_valid_name(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes.next().is_some_and(is_name_start)
        && bytes.all(is_name_continue)
        && !matches!(
            ContextualSpelling::from_spelling(text),
            Some(ContextualSpelling::Discard | ContextualSpelling::UpperSelf)
        )
        && Keyword::from_spelling(text).is_none()
}

/// Reports whether `text` is one canonical authored directory-module segment.
///
/// Module segments use the source name grammar but intentionally restrict spelling to lowercase
/// ASCII, digits, and underscores so physical directory identity has one portable spelling.
#[must_use]
pub fn is_valid_module_segment(text: &str) -> bool {
    is_valid_name(text)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

const fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

const fn is_name_continue(byte: u8) -> bool {
    is_name_start(byte) || byte.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::{is_valid_module_segment, is_valid_name};

    #[test]
    fn validates_the_parser_name_language_without_rejecting_contextual_spellings() {
        assert!(is_valid_name("value"));
        assert!(is_valid_name("T2"));
        assert!(is_valid_name("where"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("2value"));
        assert!(!is_valid_name("two-values"));
        assert!(!is_valid_name("_"));
        assert!(!is_valid_name("Self"));
        assert!(!is_valid_name("func"));
    }

    #[test]
    fn module_segments_are_portable_lowercase_names() {
        assert!(is_valid_module_segment("parser"));
        assert!(is_valid_module_segment("utf8_text"));
        assert!(is_valid_module_segment("version2"));
        assert!(!is_valid_module_segment(""));
        assert!(!is_valid_module_segment("Parser"));
        assert!(!is_valid_module_segment("_"));
        assert!(!is_valid_module_segment("func"));
        assert!(!is_valid_module_segment("two-values"));
    }
}
