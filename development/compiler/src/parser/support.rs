use crate::ast::{BinaryOperator, SwitchPayloadPattern, TypeExpr, UnaryOperator};
use crate::source::ByteSpan;

#[derive(Debug, Clone)]
pub(super) struct ParsedIdentifier {
    pub(super) value: String,
    pub(super) span: ByteSpan,
}

#[derive(Debug, Clone)]
pub(super) struct ParsedEnumPattern {
    pub(super) span: ByteSpan,
    pub(super) enum_name: String,
    pub(super) enum_name_span: ByteSpan,
    pub(super) variant_name: String,
    pub(super) variant_name_span: ByteSpan,
    pub(super) payload: Option<SwitchPayloadPattern>,
}

#[derive(Debug, Clone)]
pub(super) struct ParsedBinaryOperator {
    pub(super) value: BinaryOperator,
    pub(super) span: ByteSpan,
}

pub(super) struct ParsedUnaryOperator {
    pub(super) value: UnaryOperator,
    pub(super) span: ByteSpan,
}

pub(super) fn with_type_span(ty: TypeExpr, span: ByteSpan) -> TypeExpr {
    match ty {
        TypeExpr::Reference(mut ty) => {
            ty.span = span;
            TypeExpr::Reference(ty)
        }
        TypeExpr::Generic(mut ty) => {
            ty.span = span;
            TypeExpr::Generic(ty)
        }
        TypeExpr::Pointer(mut ty) => {
            ty.span = span;
            TypeExpr::Pointer(ty)
        }
        TypeExpr::Borrow(mut ty) => {
            ty.span = span;
            TypeExpr::Borrow(ty)
        }
        TypeExpr::View(mut ty) => {
            ty.span = span;
            TypeExpr::View(ty)
        }
        TypeExpr::Array(mut ty) => {
            ty.span = span;
            TypeExpr::Array(ty)
        }
        TypeExpr::Optional(mut ty) => {
            ty.span = span;
            TypeExpr::Optional(ty)
        }
        TypeExpr::Fallible(mut ty) => {
            ty.span = span;
            TypeExpr::Fallible(ty)
        }
    }
}
