use std::fmt;
use std::path::PathBuf;

/// One non-empty URI retained exactly as authored by the LSP client.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DocumentUri(Box<str>);

impl DocumentUri {
    /// Retains one non-empty document URI without assigning filesystem meaning.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty value.
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, DocumentUriError> {
        let value = value.into();
        if value.is_empty() {
            Err(DocumentUriError::new(DocumentUriErrorKind::Empty))
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        &self.0
    }

    /// Decodes a local absolute `file:` URI into a platform path.
    ///
    /// This operation performs no filesystem access and does not claim the result is canonical.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-file scheme, remote authority, query or fragment, malformed
    /// percent escape, NUL, non-UTF-8 escaped path, or non-absolute result.
    pub fn file_path(&self) -> Result<PathBuf, DocumentUriError> {
        let Some((scheme, remainder)) = self.0.split_once(':') else {
            return Err(DocumentUriError::new(DocumentUriErrorKind::MissingScheme));
        };
        if !scheme.eq_ignore_ascii_case("file") {
            return Err(DocumentUriError::new(DocumentUriErrorKind::NonFileScheme));
        }
        if remainder.contains(['?', '#']) {
            return Err(DocumentUriError::new(DocumentUriErrorKind::QueryOrFragment));
        }

        let path = if let Some(authority_and_path) = remainder.strip_prefix("//") {
            let slash = authority_and_path
                .find('/')
                .ok_or_else(|| DocumentUriError::new(DocumentUriErrorKind::MissingAbsolutePath))?;
            let authority = &authority_and_path[..slash];
            if !authority.is_empty() && !authority.eq_ignore_ascii_case("localhost") {
                return Err(DocumentUriError::new(DocumentUriErrorKind::RemoteAuthority));
            }
            &authority_and_path[slash..]
        } else {
            remainder
        };
        if !path.starts_with('/') {
            return Err(DocumentUriError::new(
                DocumentUriErrorKind::MissingAbsolutePath,
            ));
        }

        let decoded = percent_decode(path)?;
        let decoded = String::from_utf8(decoded)
            .map_err(|_| DocumentUriError::new(DocumentUriErrorKind::NonUtf8Path))?;
        let path = platform_path(decoded);
        if !path.is_absolute() {
            return Err(DocumentUriError::new(
                DocumentUriErrorKind::MissingAbsolutePath,
            ));
        }
        Ok(path)
    }
}

#[cfg(not(windows))]
fn platform_path(decoded: String) -> PathBuf {
    PathBuf::from(decoded)
}

#[cfg(windows)]
fn platform_path(mut decoded: String) -> PathBuf {
    let bytes = decoded.as_bytes();
    if bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
        decoded.remove(0);
    }
    PathBuf::from(decoded)
}

fn percent_decode(path: &str) -> Result<Vec<u8>, DocumentUriError> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'%' {
            decoded.push(bytes[cursor]);
            cursor += 1;
            continue;
        }
        let high = bytes
            .get(cursor + 1)
            .copied()
            .and_then(hex_digit)
            .ok_or_else(|| DocumentUriError::new(DocumentUriErrorKind::InvalidPercentEscape))?;
        let low = bytes
            .get(cursor + 2)
            .copied()
            .and_then(hex_digit)
            .ok_or_else(|| DocumentUriError::new(DocumentUriErrorKind::InvalidPercentEscape))?;
        let byte = high * 16 + low;
        if byte == 0 {
            return Err(DocumentUriError::new(DocumentUriErrorKind::NulPath));
        }
        decoded.push(byte);
        cursor += 3;
    }
    Ok(decoded)
}

const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentUriErrorKind {
    Empty,
    MissingScheme,
    NonFileScheme,
    RemoteAuthority,
    QueryOrFragment,
    MissingAbsolutePath,
    InvalidPercentEscape,
    NulPath,
    NonUtf8Path,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentUriError {
    kind: DocumentUriErrorKind,
}

impl DocumentUriError {
    const fn new(kind: DocumentUriErrorKind) -> Self {
        Self { kind }
    }

    #[must_use]
    pub const fn kind(self) -> DocumentUriErrorKind {
        self.kind
    }
}

impl fmt::Display for DocumentUriError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            DocumentUriErrorKind::Empty => "document URI is empty",
            DocumentUriErrorKind::MissingScheme => "document URI has no scheme",
            DocumentUriErrorKind::NonFileScheme => "document URI is not a file URI",
            DocumentUriErrorKind::RemoteAuthority => "document file URI has a non-local authority",
            DocumentUriErrorKind::QueryOrFragment => {
                "document file URI contains a query or fragment"
            }
            DocumentUriErrorKind::MissingAbsolutePath => "document file URI has no absolute path",
            DocumentUriErrorKind::InvalidPercentEscape => {
                "document file URI contains an invalid percent escape"
            }
            DocumentUriErrorKind::NulPath => "document file URI contains NUL",
            DocumentUriErrorKind::NonUtf8Path => "document file URI path is not encoded as UTF-8",
        })
    }
}

impl std::error::Error for DocumentUriError {}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn decodes_local_absolute_file_uris_exactly() {
        let uri = DocumentUri::new("file:///tmp/Nocter%20β/app.nct").unwrap();
        assert_eq!(uri.file_path().unwrap(), Path::new("/tmp/Nocter β/app.nct"));

        let localhost = DocumentUri::new("FILE://localhost/tmp/app.nct").unwrap();
        assert_eq!(localhost.file_path().unwrap(), Path::new("/tmp/app.nct"));
    }

    #[test]
    fn rejects_nonlocal_or_ambiguous_file_locations() {
        let cases = [
            ("", DocumentUriErrorKind::Empty),
            ("untitled:Nocter", DocumentUriErrorKind::NonFileScheme),
            (
                "file://server/share/app.nct",
                DocumentUriErrorKind::RemoteAuthority,
            ),
            (
                "file:///tmp/app.nct?version=1",
                DocumentUriErrorKind::QueryOrFragment,
            ),
            (
                "file:///tmp/%GG.nct",
                DocumentUriErrorKind::InvalidPercentEscape,
            ),
            (
                "file:relative.nct",
                DocumentUriErrorKind::MissingAbsolutePath,
            ),
        ];
        for (source, expected) in cases {
            let error = match DocumentUri::new(source) {
                Ok(uri) => uri.file_path().unwrap_err(),
                Err(error) => error,
            };
            assert_eq!(error.kind(), expected, "unexpected result for {source}");
        }
    }
}
