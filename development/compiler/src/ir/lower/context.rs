//! Runtime names, call signatures, and parameter ABI storage used by MIR projection.

use crate::abi::ReturnPassing;
use crate::ast::TypeExpr;
use crate::ir::{CallTarget, Type};
use crate::semantic::DefId;
use crate::typecheck::TypedHir;
use std::collections::HashMap;

use super::errors::ErrorPayload;

pub(super) type ErrorPayloads = HashMap<CallTarget, ErrorPayload>;
pub(super) type ResolvedSources<'a> = crate::resolve::ResolvedSources<'a>;

#[derive(Debug, Clone, Default)]
pub(super) struct FunctionNames {
    by_definition: HashMap<DefId, CallTarget>,
    by_instance: crate::mir::MonoItemRegistry<CallTarget>,
    drops_by_definition_and_type: HashMap<(DefId, crate::semantic::TypeIdentity), CallTarget>,
}

impl FunctionNames {
    pub(super) fn from_index(
        functions: Vec<(DefId, CallTarget)>,
        instances: Vec<(crate::mir::CallInstanceKey, CallTarget)>,
        drops: Vec<(DefId, TypeExpr, CallTarget)>,
        indexed_targets: &std::collections::HashSet<CallTarget>,
    ) -> Self {
        Self {
            by_definition: functions.into_iter().collect(),
            by_instance: crate::mir::MonoItemRegistry::from_entries(
                instances
                    .into_iter()
                    .filter(|(_, target)| indexed_targets.contains(target)),
            ),
            drops_by_definition_and_type: drops
                .into_iter()
                .map(|(definition, ty, name)| {
                    (
                        (
                            definition,
                            crate::semantic::TypeIdentity::runtime_drop_subject(&ty),
                        ),
                        name,
                    )
                })
                .collect(),
        }
    }

    fn target_for_definition(&self, definition: DefId) -> Option<&CallTarget> {
        self.by_definition.get(&definition)
    }

    pub(in crate::ir::lower) fn target_for_instance(
        &self,
        instance: &crate::mir::CallInstance,
        typed_hir: &TypedHir,
    ) -> Option<&CallTarget> {
        if instance.receiver.is_none()
            && instance.type_arguments.is_empty()
            && let crate::mir::CallableIdentity::Definition(definition) = instance.callable
        {
            return self.target_for_definition(definition);
        }
        let key = crate::mir::CallInstanceKey::from_instance(instance, typed_hir);
        self.by_instance.value_for(key.as_ref()?)
    }

    pub(in crate::ir::lower) fn target_for_drop(
        &self,
        definition: DefId,
        ty: &TypeExpr,
    ) -> Option<&CallTarget> {
        self.drops_by_definition_and_type.get(&(
            definition,
            crate::semantic::TypeIdentity::runtime_drop_subject(ty),
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct FunctionSignatures {
    signatures: HashMap<CallTarget, FunctionSignature>,
}

impl FunctionSignatures {
    pub(super) fn from_call_targets(signatures: HashMap<CallTarget, FunctionSignature>) -> Self {
        Self { signatures }
    }

    pub(super) fn return_type(&self, target: &CallTarget) -> Option<&Type> {
        self.signatures
            .get(target)
            .map(|signature| &signature.return_type)
    }

    pub(super) fn success_return_passing(&self, target: &CallTarget) -> Option<ReturnPassing> {
        self.signatures
            .get(target)
            .and_then(|signature| signature.success_return_passing)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FunctionSignature {
    pub(super) return_type: Type,
    pub(super) parameter_types: Option<Vec<Type>>,
    pub(super) parameter_abi_word_count: Option<usize>,
    pub(super) success_return_passing: Option<ReturnPassing>,
}
