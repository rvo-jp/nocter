//! Semantic callable-instance identity retained by checked MIR.
//!
//! Runtime symbol spelling is deliberately absent here. A declared callable
//! is identified by its canonical definition and concrete types; invocation
//! through a callable value is identified by its checked callable type and
//! capability. Only the machine backend projects either form to a symbol.

use crate::semantic::{DefId, TyId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CallableIdentity {
    Definition(DefId),
    Value {
        ty: TyId,
        capability: crate::ast::CallableCapability,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CallInstance {
    pub(crate) callable: CallableIdentity,
    pub(crate) receiver: Option<TyId>,
    pub(crate) type_arguments: Vec<TyId>,
}

impl CallInstance {
    pub(crate) fn direct(definition: DefId) -> Self {
        Self {
            callable: CallableIdentity::Definition(definition),
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
            callable: CallableIdentity::Definition(definition),
            receiver,
            type_arguments,
        }
    }

    pub(crate) fn value(ty: TyId, capability: crate::ast::CallableCapability) -> Self {
        Self {
            callable: CallableIdentity::Value { ty, capability },
            receiver: None,
            type_arguments: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CallableKey {
    Definition(DefId),
    Value {
        ty: String,
        capability: crate::ast::CallableCapability,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CallInstanceKey {
    callable: CallableKey,
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
            callable: CallableKey::Definition(definition),
            receiver: receiver.map(crate::ast::canonical_type_expr),
            type_arguments: type_arguments
                .into_iter()
                .map(crate::ast::canonical_type_expr)
                .collect(),
        }
    }

    pub(crate) fn from_callable_type(
        ty: &crate::ast::TypeExpr,
        capability: crate::ast::CallableCapability,
    ) -> Self {
        Self {
            callable: CallableKey::Value {
                ty: crate::ast::canonical_type_expr(ty),
                capability,
            },
            receiver: None,
            type_arguments: Vec::new(),
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
        match instance.callable {
            CallableIdentity::Definition(definition) => {
                Some(Self::from_types(definition, receiver, type_arguments))
            }
            CallableIdentity::Value { ty, capability } => {
                if receiver.is_some() || !type_arguments.is_empty() {
                    return None;
                }
                Some(Self::from_callable_type(
                    typed_hir.type_expr_by_id(ty)?,
                    capability,
                ))
            }
        }
    }
}
