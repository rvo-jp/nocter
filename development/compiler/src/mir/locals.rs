//! MIR-local type, ownership, storage, and source-identity contracts.
//!
//! These axes deliberately remain independent. A semantic type does not
//! determine where one body-local value is stored, and path-sensitive
//! initialization is not a property of the type or its source declaration.

use crate::mir::ScopeId;
use crate::resolve::LocalSymbolId;
use crate::semantic::{ExprId, TyId};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Local {
    pub(crate) ty: TyId,
    pub(crate) representation: ValueRepresentation,
    pub(crate) ownership: OwnershipKind,
    pub(crate) storage: LocalStorage,
    pub(crate) origin: LocalOrigin,
    pub(crate) scope: ScopeId,
}

impl Local {
    pub(crate) fn scalar(
        ty: TyId,
        scalar: ScalarType,
        storage: LocalStorage,
        origin: LocalOrigin,
        scope: ScopeId,
    ) -> Self {
        Self {
            ty,
            representation: ValueRepresentation::Scalar(scalar),
            ownership: OwnershipKind::Copy,
            storage,
            origin,
            scope,
        }
    }

    #[allow(
        dead_code,
        reason = "aggregate lowering will use this constructor in the next Phase 3 checkpoint"
    )]
    pub(crate) fn aggregate(
        ty: TyId,
        ownership: OwnershipKind,
        storage: LocalStorage,
        origin: LocalOrigin,
        scope: ScopeId,
    ) -> Self {
        Self {
            ty,
            representation: ValueRepresentation::Aggregate,
            ownership,
            storage,
            origin,
            scope,
        }
    }

    pub(crate) fn scalar_type(&self) -> Option<ScalarType> {
        match self.representation {
            ValueRepresentation::Scalar(scalar) => Some(scalar),
            ValueRepresentation::Aggregate => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueRepresentation {
    Scalar(ScalarType),
    Aggregate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarType {
    I32,
    Usize,
    Bool,
}

/// Static ownership behavior of a local's type. Runtime initialization and
/// drop obligations are represented separately on control-flow paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    dead_code,
    reason = "borrowed locals are introduced before their Phase 3 lowering checkpoint"
)]
pub(crate) enum OwnershipKind {
    Copy,
    Move,
    Borrowed { readwrite: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalStorage {
    Return,
    Parameter(usize),
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalOrigin {
    Return,
    Parameter(LocalSymbolId),
    Binding(LocalSymbolId),
    Temporary(ExprId),
    Desugared(ByteSpan),
}
