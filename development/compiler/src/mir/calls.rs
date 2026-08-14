//! Semantic callable-instance identity retained by checked MIR.
//!
//! Runtime symbol spelling is deliberately absent here. A generic or instance
//! callable is identified by its canonical declaration plus the concrete
//! checked types that selected its implementation. The machine backend is the
//! only layer that projects this identity to a linker-visible target.

use crate::semantic::{DefId, TyId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CallInstance {
    pub(crate) definition: DefId,
    pub(crate) receiver: Option<TyId>,
    pub(crate) type_arguments: Vec<TyId>,
}

/// Stable compile-unit lookup key for the backend projection of a call
/// instance. `TyId` is file-local, so registry keys use canonical type
/// notation while checked MIR itself retains only `TyId` values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CallInstanceKey {
    pub(crate) definition: DefId,
    receiver: Option<String>,
    type_arguments: Vec<String>,
}

impl CallInstanceKey {
    pub(crate) fn from_types<'a>(
        definition: DefId,
        receiver: Option<&crate::ast::TypeExpr>,
        type_arguments: impl IntoIterator<Item = &'a crate::ast::TypeExpr>,
    ) -> Self {
        Self {
            definition,
            receiver: receiver.map(crate::ast::canonical_type_expr),
            type_arguments: type_arguments
                .into_iter()
                .map(crate::ast::canonical_type_expr)
                .collect(),
        }
    }

    pub(crate) fn from_instance(
        instance: &CallInstance,
        typed_hir: &crate::typecheck::TypedHir,
    ) -> Option<Self> {
        let receiver = match instance.receiver {
            Some(ty) => Some(typed_hir.type_expr_by_id(ty)?),
            None => None,
        };
        let type_arguments = instance
            .type_arguments
            .iter()
            .map(|ty| typed_hir.type_expr_by_id(*ty))
            .collect::<Option<Vec<_>>>()?;
        Some(Self::from_types(
            instance.definition,
            receiver,
            type_arguments,
        ))
    }
}

impl CallInstance {
    pub(crate) fn direct(definition: DefId) -> Self {
        Self {
            definition,
            receiver: None,
            type_arguments: Vec::new(),
        }
    }

    pub(crate) fn specialized(
        definition: DefId,
        receiver: Option<TyId>,
        type_arguments: Vec<TyId>,
    ) -> Self {
        Self {
            definition,
            receiver,
            type_arguments,
        }
    }
}
