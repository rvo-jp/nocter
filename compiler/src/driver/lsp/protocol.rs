use crate::source::ByteSpan;
use serde::Serialize;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

#[derive(Debug, Clone, Serialize)]
pub(super) struct LspRange {
    pub(super) start: LspPosition,
    pub(super) end: LspPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct LspPosition {
    pub(super) line: usize,
    pub(super) character: usize,
}

pub(super) fn response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

pub(super) fn range_for_byte_span(text: &str, span: ByteSpan) -> LspRange {
    LspRange {
        start: byte_offset_to_lsp_position(text, span.start),
        end: byte_offset_to_lsp_position(text, span.end),
    }
}

pub(super) fn position_from_params(params: Option<&Value>) -> Option<LspPosition> {
    let position = params?.get("position")?;
    Some(LspPosition {
        line: position.get("line")?.as_u64()? as usize,
        character: position.get("character")?.as_u64()? as usize,
    })
}

pub(super) fn lsp_position_to_byte_offset(text: &str, line: usize, character: usize) -> usize {
    let mut current_line = 0;
    let mut line_start = 0;

    for (index, byte) in text.bytes().enumerate() {
        if current_line == line {
            break;
        }
        if byte == b'\n' {
            current_line += 1;
            line_start = index + 1;
        }
    }

    if current_line != line {
        return text.len();
    }

    let line_end = text[line_start..]
        .find('\n')
        .map(|offset| line_start + offset)
        .unwrap_or(text.len());
    let mut utf16_character = 0;

    for (offset, char) in text[line_start..line_end].char_indices() {
        if utf16_character >= character {
            return line_start + offset;
        }
        utf16_character += char.len_utf16();
        if utf16_character > character {
            return line_start + offset;
        }
    }

    line_end
}

pub(super) fn byte_offset_to_lsp_position(text: &str, offset: usize) -> LspPosition {
    let offset = offset.min(text.len());
    let mut line = 0;
    let mut line_start = 0;

    for (index, byte) in text.bytes().enumerate() {
        if index >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }

    let character = text
        .get(line_start..offset)
        .map(|line_text| line_text.encode_utf16().count())
        .unwrap_or(0);

    LspPosition { line, character }
}

pub(super) fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3])
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            output.push(byte);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

pub(super) fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }

    let Some(content_length) = content_length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LSP message missing Content-Length header",
        ));
    };

    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map(Some).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid LSP JSON message: {error}"),
        )
    })
}

pub(super) fn write_message<W: Write>(writer: &mut W, message: Value) -> io::Result<()> {
    let body = serde_json::to_vec(&message).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to serialize LSP message: {error}"),
        )
    })?;

    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}
