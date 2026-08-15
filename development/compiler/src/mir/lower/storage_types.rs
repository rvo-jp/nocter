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
    _initializer: &Expr,
    semantic: SemanticInputs<'_>,
) -> TyId {
    normalized_storage_type(declared, semantic)
}

pub(super) fn normalized_storage_type(ty: TyId, semantic: SemanticInputs<'_>) -> TyId {
    let Some(authored) = semantic.typed_hir.type_expr_by_id(ty) else {
        return ty;
    };
    crate::typecheck::normalize_associated_type_expr(authored, semantic.resolved)
        .and_then(|normalized| semantic.typed_hir.type_id(&normalized))
        .unwrap_or(ty)
}

/// Canonical checked identity for an ABI scalar.
///
/// Built-in scalar identities are always present in the typed arena.  This is
/// the storage-side fallback for declarations whose authored type is hidden
/// behind an imported private alias.
pub(super) fn scalar_type_id_from_abi(
    abi: &crate::abi::AbiType,
    span: crate::source::ByteSpan,
    semantic: SemanticInputs<'_>,
) -> Option<TyId> {
    let name = match abi {
        crate::abi::AbiType::Bool => "bool",
        integer if integer.integer_type().is_some() => integer.integer_type()?.name(),
        _ => return None,
    };
    semantic.typed_hir.type_id(&crate::ast::TypeExpr::Reference(
        crate::ast::TypeReference {
            span,
            name: name.to_string(),
        },
    ))
}

/// Resolves a source-level type to the identity carried by its runtime value.
///
/// Opaque result types deliberately hide their witness from source callers,
/// but monomorphized MIR must dispatch methods against that concrete witness.
/// Associated projections are normalized for the same reason. Keeping this
/// translation at the storage boundary avoids teaching individual call
/// lowerers about every transparent source-level wrapper.
pub(super) fn runtime_type_id_for_type_expr(
    ty: &crate::ast::TypeExpr,
    semantic: SemanticInputs<'_>,
) -> Option<TyId> {
    if let Some(id) = semantic.typed_hir.type_id(ty) {
        return Some(normalized_storage_type(id, semantic));
    }
    if let crate::ast::TypeExpr::Opaque(opaque) = ty
        && let Some(witness) = &opaque.witness
    {
        return runtime_type_id_for_type_expr(witness, semantic);
    }
    crate::typecheck::normalize_associated_type_expr(ty, semantic.resolved)
        .and_then(|normalized| semantic.typed_hir.type_id(&normalized))
        .map(|id| normalized_storage_type(id, semantic))
}
