use super::*;

impl PackageFile {
    pub fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::new(
            "package_file",
            json_span(sources, self.span),
            self.manifest
                .directives
                .iter()
                .map(|directive| directive.to_json(sources))
                .chain(std::iter::once(self.root_module.to_json(sources)))
                .collect(),
        )
    }
}

impl PackageDirective {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "package_directive",
            self.name.clone(),
            json_span(sources, self.span),
            vec![self.value.to_json(sources)],
        )
    }
}

impl DirectiveValue {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            Self::String { span, value, .. } => JsonAstNode::with_value(
                "directive_string",
                value.clone(),
                json_span(sources, *span),
                Vec::new(),
            ),
            Self::Integer { span, value } => JsonAstNode::with_value(
                "directive_integer",
                value.to_string(),
                json_span(sources, *span),
                Vec::new(),
            ),
            Self::Boolean { span, value } => JsonAstNode::with_value(
                "directive_boolean",
                value.to_string(),
                json_span(sources, *span),
                Vec::new(),
            ),
            Self::List { span, values } => JsonAstNode::new(
                "directive_list",
                json_span(sources, *span),
                values.iter().map(|value| value.to_json(sources)).collect(),
            ),
            Self::Record { span, fields } => JsonAstNode::new(
                "directive_record",
                json_span(sources, *span),
                fields.iter().map(|field| field.to_json(sources)).collect(),
            ),
        }
    }
}

impl DirectiveField {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "directive_field",
            self.name.clone(),
            json_span(sources, self.span),
            vec![self.value.to_json(sources)],
        )
    }
}
