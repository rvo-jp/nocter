//! Semantic callable-instance identity retained by checked MIR.
//!
//! Runtime symbol spelling is deliberately absent here. A declared callable
//! is identified by its canonical definition and concrete types; invocation
//! through a callable value is identified by its checked callable type and
//! capability. Only the machine backend projects either form to a symbol.

use crate::semantic::{DefId, TyId};
use std::collections::HashMap;

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
        ty: crate::semantic::TypeIdentity,
        capability: crate::ast::CallableCapability,
    },
    Literal {
        definition: DefId,
        shape: crate::ast::LiteralShape,
        result: crate::semantic::TypeIdentity,
        segments: Vec<LiteralSegmentKey>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum LiteralSegmentKey {
    Value,
    Spread {
        mode: crate::typecheck::TypecheckSequenceSpreadMode,
        iterator: crate::semantic::TypeIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CallInstanceKey {
    callable: CallableKey,
    receiver: Option<crate::semantic::TypeIdentity>,
    type_arguments: Vec<crate::semantic::TypeIdentity>,
}

impl CallInstanceKey {
    pub(crate) fn from_types<'a>(
        definition: DefId,
        receiver: Option<&crate::ast::TypeExpr>,
        type_arguments: impl IntoIterator<Item = &'a crate::ast::TypeExpr>,
    ) -> Self {
        Self {
            callable: CallableKey::Definition(definition),
            receiver: receiver.map(crate::semantic::TypeIdentity::call_receiver),
            type_arguments: type_arguments
                .into_iter()
                .map(crate::semantic::TypeIdentity::of)
                .collect(),
        }
    }

    pub(crate) fn from_callable_type(
        ty: &crate::ast::TypeExpr,
        capability: crate::ast::CallableCapability,
    ) -> Self {
        Self {
            callable: CallableKey::Value {
                ty: crate::semantic::TypeIdentity::of(ty),
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
                result: crate::semantic::TypeIdentity::of(result),
                segments: segments
                    .into_iter()
                    .map(|(mode, iterator)| match (mode, iterator) {
                        (None, None) => LiteralSegmentKey::Value,
                        (Some(mode), Some(iterator)) => LiteralSegmentKey::Spread {
                            mode,
                            iterator: crate::semantic::TypeIdentity::of(iterator),
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

/// Exact compile-unit registry for monomorphized callable identities.
///
/// A `MonoItemId` can only be obtained from a complete structural key. There
/// is intentionally no operation that erases a receiver or type argument.
#[derive(Debug, Clone)]
pub(crate) struct MonoItemRegistry<T> {
    ids: HashMap<CallInstanceKey, crate::semantic::MonoItemId>,
    entries: Vec<(CallInstanceKey, T)>,
}

impl<T> Default for MonoItemRegistry<T> {
    fn default() -> Self {
        Self {
            ids: HashMap::new(),
            entries: Vec::new(),
        }
    }
}

impl<T> MonoItemRegistry<T> {
    pub(crate) fn from_entries(entries: impl IntoIterator<Item = (CallInstanceKey, T)>) -> Self {
        let mut registry = Self::default();
        for (key, value) in entries {
            registry.insert(key, value);
        }
        registry
    }

    pub(crate) fn insert(&mut self, key: CallInstanceKey, value: T) -> crate::semantic::MonoItemId {
        if let Some(id) = self.ids.get(&key) {
            return *id;
        }
        let id = crate::semantic::MonoItemId::from_index(self.entries.len());
        self.ids.insert(key.clone(), id);
        self.entries.push((key, value));
        id
    }

    pub(crate) fn resolve(&self, key: &CallInstanceKey) -> Option<crate::semantic::MonoItemId> {
        self.ids.get(key).copied()
    }

    pub(crate) fn get(&self, id: crate::semantic::MonoItemId) -> Option<&T> {
        self.entries.get(id.index()).map(|(_, value)| value)
    }

    pub(crate) fn value_for(&self, key: &CallInstanceKey) -> Option<&T> {
        self.get(self.resolve(key)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{GenericType, TypeExpr, TypeReference};
    use crate::source::{ByteSpan, SourceId};

    fn reference(name: &str) -> TypeExpr {
        TypeExpr::Reference(TypeReference {
            span: ByteSpan::new(SourceId::new(0), 0, 1),
            name: name.to_string(),
        })
    }

    #[test]
    fn registry_requires_an_exact_structured_instance_key() {
        let definition = DefId::from_index(4);
        let receiver = TypeExpr::Generic(GenericType {
            span: ByteSpan::new(SourceId::new(0), 0, 1),
            name: "package.Box".to_string(),
            name_span: ByteSpan::new(SourceId::new(0), 0, 1),
            arguments: vec![reference("i32")],
        });
        let key = CallInstanceKey::from_types(definition, Some(&receiver), [&reference("bool")]);
        let different =
            CallInstanceKey::from_types(definition, Some(&receiver), [&reference("usize")]);
        let mut registry = MonoItemRegistry::default();
        let id = registry.insert(key.clone(), "Box<bool>.call");
        assert_eq!(registry.resolve(&key), Some(id));
        assert_eq!(registry.value_for(&key), Some(&"Box<bool>.call"));
        assert_eq!(registry.resolve(&different), None);
    }
}
