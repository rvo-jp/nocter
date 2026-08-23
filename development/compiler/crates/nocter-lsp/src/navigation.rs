use nocter_json::{Member, Value};

use crate::coordinates::range_value;
use crate::decode::{Object, boolean, required};
use crate::{DocumentUri, ParameterError, Position, Range, TextDocumentPositionParams};

pub type DefinitionParams = TextDocumentPositionParams;
pub type ImplementationParams = TextDocumentPositionParams;

/// Validated `textDocument/references` parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferencesParams {
    document: TextDocumentPositionParams,
    include_declaration: bool,
}

impl ReferencesParams {
    /// Decodes one references request and its required inclusion policy.
    ///
    /// # Errors
    ///
    /// Returns the exact missing, duplicate, or incorrectly typed field.
    pub fn decode(params: Option<Value>) -> Result<Self, ParameterError> {
        let mut root = Object::new(required(params, "params")?, "params")?;
        let document = TextDocumentPositionParams::decode_from(&mut root)?;
        let mut context = Object::new(root.take("context")?, "params.context")?;
        let include_declaration = boolean(
            &context.take("includeDeclaration")?,
            "params.context.includeDeclaration",
        )?;
        Ok(Self {
            document,
            include_declaration,
        })
    }

    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        self.document.uri()
    }

    #[must_use]
    pub const fn position(&self) -> Position {
        self.document.position()
    }

    #[must_use]
    pub const fn include_declaration(&self) -> bool {
        self.include_declaration
    }
}

/// One protocol location projected from a compiler-owned source range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    uri: DocumentUri,
    range: Range,
}

impl Location {
    #[must_use]
    pub const fn new(uri: DocumentUri, range: Range) -> Self {
        Self { uri, range }
    }
}

/// Renders a deterministic location array for definition, implementation, or references.
#[must_use]
pub fn locations_result(locations: &[Location]) -> Value {
    Value::Array(
        locations
            .iter()
            .map(|location| {
                object([
                    ("uri", Value::String(location.uri.as_str().into())),
                    ("range", range_value(location.range)),
                ])
            })
            .collect(),
    )
}

fn object<const N: usize>(members: [(&str, Value); N]) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| Member {
                name: name.into(),
                value,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use nocter_json::{parse, write_value};

    use super::*;

    #[test]
    fn decodes_reference_context_with_the_shared_position_boundary() {
        let params = ReferencesParams::decode(Some(
            parse(concat!(
                "{\"textDocument\":{\"uri\":\"file:///workspace/main.nct\"},",
                "\"position\":{\"line\":2,\"character\":7},",
                "\"context\":{\"includeDeclaration\":true}}"
            ))
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(params.position(), Position::new(2, 7));
        assert!(params.include_declaration());
    }

    #[test]
    fn renders_location_arrays() {
        let mut rendered = String::new();
        write_value(
            &mut rendered,
            &locations_result(&[Location::new(
                DocumentUri::new("file:///workspace/main.nct").unwrap(),
                Range::new(Position::new(1, 2), Position::new(1, 6)),
            )]),
        );
        assert_eq!(
            rendered,
            concat!(
                "[{\"uri\":\"file:///workspace/main.nct\",\"range\":{",
                "\"start\":{\"line\":1,\"character\":2},",
                "\"end\":{\"line\":1,\"character\":6}}}]"
            )
        );
    }
}
