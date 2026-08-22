use std::fmt;

use nocter_analysis::SourceContextError;
use nocter_json::Value;
use nocter_lsp::{SignatureHelpParams, SignatureParameter, signature_help_result};
use nocter_source::{CoordinateError, Utf16Position};

use crate::semantic_document::{SemanticDocumentError, semantic_document};
use crate::{DocumentWorkspace, WorkspaceAnalyses};

/// Answers signature help from the innermost checked call at the requested position.
pub(crate) fn query_signature_help(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &SignatureHelpParams,
) -> Result<Value, SignatureQueryError> {
    let Some(document) = semantic_document(documents, analyses, params.uri())
        .map_err(SignatureQueryError::Document)?
    else {
        return Ok(Value::Null);
    };
    let offset = document
        .source()
        .byte_offset(Utf16Position::new(
            params.position().line(),
            params.position().character(),
        ))
        .map_err(SignatureQueryError::Coordinate)?;
    document
        .snapshot()
        .semantic_signature_help(document.source().id(), offset)
        .map_err(SignatureQueryError::Semantic)?
        .map_or_else(
            || Ok(Value::Null),
            |help| {
                let label = help.presentation().code();
                let parameters = help
                    .parameters()
                    .iter()
                    .map(|parameter| {
                        Ok(SignatureParameter::new(
                            utf16_offset(label, parameter.start())?,
                            utf16_offset(label, parameter.end())?,
                        ))
                    })
                    .collect::<Result<Vec<_>, SignatureQueryError>>()?;
                Ok(signature_help_result(
                    label,
                    &parameters,
                    help.active_parameter(),
                ))
            },
        )
}

fn utf16_offset(label: &str, byte: usize) -> Result<u32, SignatureQueryError> {
    let prefix = label
        .get(..byte)
        .ok_or(SignatureQueryError::InvalidLabelRange(byte))?;
    u32::try_from(prefix.encode_utf16().count())
        .map_err(|_| SignatureQueryError::InvalidLabelRange(byte))
}

#[derive(Debug)]
pub enum SignatureQueryError {
    Document(SemanticDocumentError),
    Coordinate(CoordinateError),
    Semantic(SourceContextError),
    InvalidLabelRange(usize),
}

impl fmt::Display for SignatureQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(formatter),
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Semantic(error) => error.fmt(formatter),
            Self::InvalidLabelRange(offset) => {
                write!(
                    formatter,
                    "signature label range is invalid at byte {offset}"
                )
            }
        }
    }
}

impl std::error::Error for SignatureQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::Coordinate(error) => Some(error),
            Self::Semantic(error) => Some(error),
            Self::InvalidLabelRange(_) => None,
        }
    }
}
