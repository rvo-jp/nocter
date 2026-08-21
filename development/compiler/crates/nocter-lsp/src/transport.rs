use std::fmt;
use std::io::{self, BufRead, Write};

const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Stateful reader for LSP `Content-Length` frames.
pub struct FrameReader<R> {
    input: R,
}

impl<R: BufRead> FrameReader<R> {
    #[must_use]
    pub const fn new(input: R) -> Self {
        Self { input }
    }

    /// Reads one UTF-8 JSON body, or `None` for clean EOF between messages.
    ///
    /// # Errors
    ///
    /// Returns an I/O, header-shape, size-limit, truncation, or UTF-8 failure.
    pub fn read(&mut self) -> Result<Option<String>, FrameError> {
        let mut content_length = None;
        let mut header_bytes = 0_usize;
        loop {
            let Some(mut line) = self.read_header_line(header_bytes)? else {
                return if header_bytes == 0 {
                    Ok(None)
                } else {
                    Err(FrameError::TruncatedHeader)
                };
            };
            header_bytes = header_bytes
                .checked_add(line.len())
                .ok_or(FrameError::HeaderTooLarge)?;
            if !line.ends_with(b"\r\n") {
                return Err(FrameError::InvalidHeaderLineEnding);
            }
            line.truncate(line.len() - 2);
            if line.is_empty() {
                break;
            }
            let line = std::str::from_utf8(&line).map_err(|_| FrameError::NonAsciiHeader)?;
            if !line.is_ascii() {
                return Err(FrameError::NonAsciiHeader);
            }
            let Some((name, value)) = line.split_once(':') else {
                return Err(FrameError::MalformedHeader);
            };
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            {
                return Err(FrameError::MalformedHeader);
            }
            if name.eq_ignore_ascii_case("Content-Length") {
                if content_length.is_some() {
                    return Err(FrameError::DuplicateContentLength);
                }
                let value = value.trim();
                if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(FrameError::InvalidContentLength);
                }
                let length = value
                    .parse::<usize>()
                    .map_err(|_| FrameError::InvalidContentLength)?;
                if length > MAX_MESSAGE_BYTES {
                    return Err(FrameError::MessageTooLarge {
                        bytes: length,
                        limit: MAX_MESSAGE_BYTES,
                    });
                }
                content_length = Some(length);
            }
        }

        let length = content_length.ok_or(FrameError::MissingContentLength)?;
        let mut body = vec![0; length];
        self.input
            .read_exact(&mut body)
            .map_err(|error| match error.kind() {
                io::ErrorKind::UnexpectedEof => FrameError::TruncatedBody,
                _ => FrameError::Io(error),
            })?;
        String::from_utf8(body)
            .map(Some)
            .map_err(|_| FrameError::InvalidUtf8)
    }

    fn read_header_line(&mut self, consumed: usize) -> Result<Option<Vec<u8>>, FrameError> {
        let mut line = Vec::new();
        loop {
            let available = self.input.fill_buf()?;
            if available.is_empty() {
                return if line.is_empty() {
                    Ok(None)
                } else {
                    Err(FrameError::TruncatedHeader)
                };
            }
            let take = available
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(available.len(), |index| index + 1);
            if consumed
                .checked_add(line.len())
                .and_then(|total| total.checked_add(take))
                .is_none_or(|total| total > MAX_HEADER_BYTES)
            {
                return Err(FrameError::HeaderTooLarge);
            }
            line.extend_from_slice(&available[..take]);
            self.input.consume(take);
            if line.ends_with(b"\n") {
                return Ok(Some(line));
            }
        }
    }
}

/// Writes one complete LSP frame without adding protocol-external stdout text.
///
/// # Errors
///
/// Returns an output I/O failure.
pub fn write_frame(mut output: impl Write, body: &str) -> io::Result<()> {
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(body.as_bytes())?;
    output.flush()
}

#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    TruncatedHeader,
    HeaderTooLarge,
    InvalidHeaderLineEnding,
    NonAsciiHeader,
    MalformedHeader,
    MissingContentLength,
    DuplicateContentLength,
    InvalidContentLength,
    MessageTooLarge { bytes: usize, limit: usize },
    TruncatedBody,
    InvalidUtf8,
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::TruncatedHeader => formatter.write_str("truncated LSP header"),
            Self::HeaderTooLarge => formatter.write_str("LSP header exceeds the size limit"),
            Self::InvalidHeaderLineEnding => {
                formatter.write_str("LSP headers require CRLF line endings")
            }
            Self::NonAsciiHeader => formatter.write_str("LSP header is not ASCII"),
            Self::MalformedHeader => formatter.write_str("malformed LSP header"),
            Self::MissingContentLength => formatter.write_str("missing LSP Content-Length header"),
            Self::DuplicateContentLength => {
                formatter.write_str("duplicate LSP Content-Length header")
            }
            Self::InvalidContentLength => formatter.write_str("invalid LSP Content-Length value"),
            Self::MessageTooLarge { bytes, limit } => {
                write!(formatter, "LSP message size {bytes} exceeds limit {limit}")
            }
            Self::TruncatedBody => formatter.write_str("truncated LSP message body"),
            Self::InvalidUtf8 => formatter.write_str("LSP message body is not UTF-8"),
        }
    }
}

impl std::error::Error for FrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for FrameError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn reads_consecutive_frames_and_clean_eof() {
        let input = b"Content-Length: 2\r\n\r\n{}Content-Type: application/vscode-jsonrpc; charset=utf-8\r\nContent-Length: 4\r\n\r\nnull";
        let mut reader = FrameReader::new(Cursor::new(input));
        assert_eq!(reader.read().unwrap().as_deref(), Some("{}"));
        assert_eq!(reader.read().unwrap().as_deref(), Some("null"));
        assert_eq!(reader.read().unwrap(), None);
    }

    #[test]
    fn rejects_ambiguous_or_truncated_frames() {
        assert!(matches!(
            read_error(b"Content-Type: x\r\n\r\n"),
            FrameError::MissingContentLength
        ));
        assert!(matches!(
            read_error(b"Content-Length: 1\r\nContent-Length: 1\r\n\r\nx"),
            FrameError::DuplicateContentLength
        ));
        assert!(matches!(
            read_error(b"Content-Length: 2\n\n{}"),
            FrameError::InvalidHeaderLineEnding
        ));
        assert!(matches!(
            read_error(b"Content-Length: 2\r\n\r\n{"),
            FrameError::TruncatedBody
        ));
    }

    fn read_error(input: &[u8]) -> FrameError {
        FrameReader::new(Cursor::new(input)).read().unwrap_err()
    }

    #[test]
    fn writes_exact_content_length_frame() {
        let mut output = Vec::new();
        write_frame(&mut output, "{\"text\":\"β\"}").unwrap();
        assert_eq!(output, b"Content-Length: 13\r\n\r\n{\"text\":\"\xce\xb2\"}");
    }
}
