//! Semantic callable-instance identity retained by checked MIR.
//!
//! Runtime symbol spelling is deliberately absent here. A declared callable
//! is identified by its canonical definition and concrete types; invocation
//! through a callable value is identified by its checked callable type and
//! capability. Only the machine backend projects either form to a symbol.

use crate::semantic::{DefId, TyId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum CallableIdentity {
    Intrinsic(crate::intrinsics::IntrinsicId),
    Definition(DefId),
    Value {
        ty: TyId,
        capability: crate::ast::CallableCapability,
    },
    Literal {
        definition: DefId,
        shape: crate::ast::LiteralShape,
        result: TyId,
        segments: Vec<LiteralSegment>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LiteralSegment {
    Value,
    Spread {
        mode: crate::typecheck::TypecheckSequenceSpreadMode,
        iterator: TyId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CallInstance {
    pub(crate) callable: CallableIdentity,
    pub(crate) receiver: Option<TyId>,
    pub(crate) type_arguments: Vec<TyId>,
}

impl CallInstance {
    pub(crate) fn intrinsic(intrinsic: crate::intrinsics::IntrinsicId) -> Self {
        Self {
            callable: CallableIdentity::Intrinsic(intrinsic),
            receiver: None,
            type_arguments: Vec::new(),
        }
    }

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

    pub(crate) fn literal(
        definition: DefId,
        shape: crate::ast::LiteralShape,
        result: TyId,
        segments: Vec<LiteralSegment>,
    ) -> Self {
        Self {
            callable: CallableIdentity::Literal {
                definition,
                shape,
                result,
                segments,
            },
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
    Literal {
        definition: DefId,
        shape: crate::ast::LiteralShape,
        result: String,
        segments: Vec<LiteralSegmentKey>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum LiteralSegmentKey {
    Value,
    Spread {
        mode: crate::typecheck::TypecheckSequenceSpreadMode,
        iterator: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CallInstanceKey {
    callable: CallableKey,
    receiver: Option<String>,
    type_arguments: Vec<String>,
}

impl CallInstanceKey {
    pub(crate) fn with_unqualified_receiver(&self) -> Self {
        let mut key = self.clone();
        key.receiver = key
            .receiver
            .map(|receiver| unqualified_type_name(&receiver));
        key
    }

    /// Produces the receiver-specialized identity shared by owner-generic
    /// protocol calls whose type arguments are already determined by the
    /// receiver.  Name indexes may use this only as an ambiguity-checked
    /// fallback: independently generic methods can still have several runtime
    /// instances for the same definition and receiver.
    pub(crate) fn without_type_arguments(&self) -> Self {
        let mut key = self.clone();
        key.type_arguments.clear();
        key
    }

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

    pub(crate) fn from_literal_types<'a>(
        definition: DefId,
        shape: crate::ast::LiteralShape,
        result: &crate::ast::TypeExpr,
        segments: impl IntoIterator<
            Item = (
                Option<crate::typecheck::TypecheckSequenceSpreadMode>,
                Option<&'a crate::ast::TypeExpr>,
            ),
        >,
    ) -> Self {
        Self {
            callable: CallableKey::Literal {
                definition,
                shape,
                result: crate::ast::canonical_type_expr(result),
                segments: segments
                    .into_iter()
                    .map(|(mode, iterator)| match (mode, iterator) {
                        (None, None) => LiteralSegmentKey::Value,
                        (Some(mode), Some(iterator)) => LiteralSegmentKey::Spread {
                            mode,
                            iterator: crate::ast::canonical_type_expr(iterator),
                        },
                        _ => unreachable!("literal segment mode and iterator must agree"),
                    })
                    .collect(),
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
        match &instance.callable {
            CallableIdentity::Intrinsic(_) => None,
            CallableIdentity::Definition(definition) => {
                Some(Self::from_types(*definition, receiver, type_arguments))
            }
            CallableIdentity::Value { ty, capability } => {
                if receiver.is_some() || !type_arguments.is_empty() {
                    return None;
                }
                Some(Self::from_callable_type(
                    typed_hir.type_expr_by_id(*ty)?,
                    *capability,
                ))
            }
            CallableIdentity::Literal {
                definition,
                shape,
                result,
                segments,
            } => {
                if receiver.is_some() || !type_arguments.is_empty() {
                    return None;
                }
                let segments = segments
                    .iter()
                    .map(|segment| match segment {
                        LiteralSegment::Value => Some((None, None)),
                        LiteralSegment::Spread { mode, iterator } => {
                            Some((Some(*mode), Some(typed_hir.type_expr_by_id(*iterator)?)))
                        }
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(Self::from_literal_types(
                    *definition,
                    *shape,
                    typed_hir.type_expr_by_id(*result)?,
                    segments,
                ))
            }
        }
    }
}

pub(crate) fn runtime_name_with_unqualified_receiver(name: &str) -> String {
    let Some((receiver, member)) = name.rsplit_once('.') else {
        return name.to_string();
    };
    if !receiver.contains('<') {
        return name.to_string();
    }
    format!("{}.{}", unqualified_type_name(receiver), member)
}

fn unqualified_type_name(name: &str) -> String {
    let name_end = name.find('<').unwrap_or(name.len());
    let (head, suffix) = name.split_at(name_end);
    let short = head.rsplit(['.', '/']).next().unwrap_or(head);
    format!("{short}{suffix}")
}
