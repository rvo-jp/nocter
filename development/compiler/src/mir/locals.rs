//! MIR-local type, ownership, storage, and source-identity contracts.
//!
//! These axes deliberately remain independent. A semantic type does not
//! determine where one body-local value is stored, and path-sensitive
//! initialization is not a property of the type or its source declaration.

use crate::mir::{DropPlanId, ScopeId};
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
    pub(crate) drop_plan: Option<DropPlanId>,
}

impl Local {
    pub(crate) fn unit(
        ty: TyId,
        storage: LocalStorage,
        origin: LocalOrigin,
        scope: ScopeId,
    ) -> Self {
        Self {
            ty,
            representation: ValueRepresentation::Unit,
            ownership: OwnershipKind::Copy,
            storage,
            origin,
            scope,
            drop_plan: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_drop_plan(mut self, plan: DropPlanId) -> Self {
        self.drop_plan = Some(plan);
        self
    }

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
            drop_plan: None,
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
            drop_plan: None,
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
            drop_plan: None,
        }
    }

    pub(crate) fn error(
        ty: TyId,
        storage: LocalStorage,
        origin: LocalOrigin,
        scope: ScopeId,
    ) -> Self {
        Self {
            ty,
            representation: ValueRepresentation::Error,
            ownership: OwnershipKind::Copy,
            storage,
            origin,
            scope,
            drop_plan: None,
        }
    }

    pub(crate) fn view(
        ty: TyId,
        kind: ViewKind,
        storage: LocalStorage,
        origin: LocalOrigin,
        scope: ScopeId,
    ) -> Self {
        Self {
            ty,
            representation: ValueRepresentation::View(kind),
            ownership: OwnershipKind::Copy,
            storage,
            origin,
            scope,
            drop_plan: None,
        }
    }

    pub(crate) fn scalar_type(&self) -> Option<ScalarType> {
        match self.representation {
            ValueRepresentation::Scalar(scalar) => Some(scalar),
            ValueRepresentation::Borrow
            | ValueRepresentation::View(_)
            | ValueRepresentation::Aggregate
            | ValueRepresentation::Error
            | ValueRepresentation::Unit => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueRepresentation {
    Unit,
    Scalar(ScalarType),
    Borrow,
    View(ViewKind),
    Error,
    Aggregate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewKind {
    Str,
    Slice,
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
    CallableReceiver(ExprId),
    Binding(LocalSymbolId),
    Temporary(ExprId),
    Desugared(ByteSpan),
}
