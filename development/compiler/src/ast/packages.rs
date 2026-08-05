use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageHeader {
    pub directives: Vec<PackageDirective>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDirective {
    pub span: ByteSpan,
    pub name_span: ByteSpan,
    pub name: String,
    pub value: DirectiveValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectiveValue {
    String {
        span: ByteSpan,
        content_span: ByteSpan,
        value: String,
    },
    Integer {
        span: ByteSpan,
        value: u128,
    },
    Boolean {
        span: ByteSpan,
        value: bool,
    },
    List {
        span: ByteSpan,
        values: Vec<DirectiveValue>,
    },
    Record {
        span: ByteSpan,
        fields: Vec<DirectiveField>,
    },
}

impl DirectiveValue {
    pub fn span(&self) -> ByteSpan {
        match self {
            Self::String { span, .. }
            | Self::Integer { span, .. }
            | Self::Boolean { span, .. }
            | Self::List { span, .. }
            | Self::Record { span, .. } => *span,
        }
    }

    pub fn string_value(&self) -> Option<(&str, ByteSpan)> {
        match self {
            Self::String {
                content_span,
                value,
                ..
            } => Some((value, *content_span)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveField {
    pub span: ByteSpan,
    pub name_span: ByteSpan,
    pub name: String,
    pub value: DirectiveValue,
}
