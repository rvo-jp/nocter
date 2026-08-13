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

    pub(crate) fn borrow(
        ty: TyId,
        readwrite: bool,
        storage: LocalStorage,
        origin: LocalOrigin,
        scope: ScopeId,
    ) -> Self {
        Self {
            ty,
            representation: ValueRepresentation::Borrow,
            ownership: OwnershipKind::Borrowed { readwrite },
            storage,
            origin,
            scope,
        }
    }

    pub(crate) fn scalar_type(&self) -> Option<ScalarType> {
        match self.representation {
            ValueRepresentation::Scalar(scalar) => Some(scalar),
            ValueRepresentation::Borrow | ValueRepresentation::Aggregate => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueRepresentation {
    Scalar(ScalarType),
    Borrow,
    Aggregate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarType {
    I32,
    U8,
    Usize,
    Integer(crate::integer::IntegerType),
    Bool,
}

/// Static ownership behavior of a local's type. Runtime initialization and
/// drop obligations are represented separately on control-flow paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OwnershipKind {
    Copy,
    Move,
    Borrowed { readwrite: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalStorage {
    Return,
    Parameter { ordinal: usize },
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
