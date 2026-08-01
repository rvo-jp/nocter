use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonAstNode {
    pub kind: String,
    pub span: Option<JsonSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_span: Option<JsonSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    pub items: Vec<JsonAstNode>,
}

impl JsonAstNode {
    pub fn new(kind: impl Into<String>, span: Option<JsonSpan>, items: Vec<JsonAstNode>) -> Self {
        Self {
            kind: kind.into(),
            span,
            operator_span: None,
            value: None,
            documentation: None,
            items,
        }
    }

    pub fn with_value(
        kind: impl Into<String>,
        value: impl Into<String>,
        span: Option<JsonSpan>,
        items: Vec<JsonAstNode>,
    ) -> Self {
        Self {
            kind: kind.into(),
            span,
            operator_span: None,
            value: Some(value.into()),
            documentation: None,
            items,
        }
    }

    pub fn with_operator_span(mut self, operator_span: Option<JsonSpan>) -> Self {
        self.operator_span = operator_span;
        self
    }

    pub(super) fn with_documentation(mut self, documentation: Option<&str>) -> Self {
        self.documentation = documentation.map(str::to_string);
        self
    }

    pub(super) fn apply_documentation(&mut self, documentation: &AttachedDocumentation) {
        if self.documentation.is_none()
            && let Some(span) = &self.span
        {
            self.documentation = documentation.get(span.start_byte).map(str::to_string);
        }

        for item in &mut self.items {
            item.apply_documentation(documentation);
        }
    }
}
