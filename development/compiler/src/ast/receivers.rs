use super::{BorrowType, Parameter, TypeExpr, TypeReference};
use crate::source::ByteSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodReceiverMode {
    Owned,
    ReadonlyBorrow,
    ReadwriteBorrow,
}

impl MethodReceiverMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::ReadonlyBorrow => "readonly_borrow",
            Self::ReadwriteBorrow => "readwrite_borrow",
        }
    }

    pub const fn source_prefix(self) -> &'static str {
        match self {
            Self::Owned => "",
            Self::ReadonlyBorrow => "&",
            Self::ReadwriteBorrow => "&+",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodReceiver {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub mode: MethodReceiverMode,
}

impl MethodReceiver {
    /// Materializes the implicit receiver type for semantic consumers. The AST
    /// stores only the source binding and receiver mode; this type expression
    /// is not a source type reference.
    pub fn implicit_parameter(&self) -> Parameter {
        let self_type = TypeExpr::Reference(TypeReference {
            span: self.name_span,
            name: "Self".to_string(),
        });
        let ty = match self.mode {
            MethodReceiverMode::Owned => self_type,
            MethodReceiverMode::ReadonlyBorrow | MethodReceiverMode::ReadwriteBorrow => {
                TypeExpr::Borrow(BorrowType {
                    span: self.span,
                    is_readwrite: self.mode == MethodReceiverMode::ReadwriteBorrow,
                    inner: Box::new(self_type),
                })
            }
        };
        Parameter {
            span: self.span,
            name: self.name.clone(),
            name_span: self.name_span,
            ty,
        }
    }
}
