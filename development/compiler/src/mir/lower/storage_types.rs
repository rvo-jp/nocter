//! Runtime storage identities for source-level bindings.
//!
//! Transparent aliases may give the same checked value distinct semantic
//! `TyId`s at its declaration and initializer sites. MIR locals retain the
//! initializer's intrinsic identity when its runtime representation agrees
//! with the declaration. This keeps later operands and specialized call
//! arguments type-consistent without weakening MIR validation for nominal
//! types that merely share an ABI layout.

use super::SemanticInputs;
use crate::ast::Expr;
use crate::semantic::TyId;

pub(super) fn binding_storage_type(
    declared: TyId,
    initializer: &Expr,
    semantic: SemanticInputs<'_>,
) -> TyId {
    let Some(intrinsic) =
        super::coverage::intrinsic_expression_type(initializer.span(), semantic.typed_hir)
    else {
        return declared;
    };
    if super::coverage::value_representation(intrinsic, semantic)
        == super::coverage::value_representation(declared, semantic)
    {
        intrinsic
    } else {
        declared
    }
}
