use std::fmt;

use nocter_declarations::{
    CallableKind, CallableOwner, DeclarationGraph, ParameterOwner, ParameterRole, Visibility,
};
use nocter_model::{
    AllocationGuarantee, BuiltinType, CallableId, CompilationTarget, TypeId, TypeKind, TypeStore,
};
use nocter_runtime_contract::{TargetServiceRole, TargetServiceValueAbi};

use crate::ToolchainSnapshot;

/// The exact part of a trusted target-service declaration that failed target closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetServiceContractRule {
    Authority,
    CallableKind,
    Visibility,
    GenericShape,
    ParameterShape,
    ResultType,
    Guarantees,
    TargetGate,
    Body,
    Requirements,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetServiceContractError {
    role: TargetServiceRole,
    callable: CallableId,
    rule: TargetServiceContractRule,
}

impl TargetServiceContractError {
    #[must_use]
    pub const fn role(self) -> TargetServiceRole {
        self.role
    }

    #[must_use]
    pub const fn callable(self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn rule(self) -> TargetServiceContractRule {
        self.rule
    }
}

impl fmt::Display for TargetServiceContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "target service {:?} violates its {:?} contract",
            self.role, self.rule
        )
    }
}

impl std::error::Error for TargetServiceContractError {}

pub(crate) fn validate_target_services(
    graph: &DeclarationGraph,
    types: &TypeStore,
    snapshot: &ToolchainSnapshot,
) -> Result<(), TargetServiceContractError> {
    for binding in snapshot.target_services().bindings() {
        validate_binding(
            graph,
            types,
            snapshot.target(),
            snapshot.standard_package(),
            binding.role(),
            binding.callable(),
            binding.descriptor(),
        )?;
    }
    Ok(())
}

fn validate_binding(
    graph: &DeclarationGraph,
    types: &TypeStore,
    target: CompilationTarget,
    standard_package: nocter_model::PackageId,
    role: TargetServiceRole,
    callable: CallableId,
    descriptor: &nocter_runtime_contract::TargetServiceDescriptor,
) -> Result<(), TargetServiceContractError> {
    let fail = |rule| TargetServiceContractError {
        role,
        callable,
        rule,
    };
    if descriptor.target() != target {
        return Err(fail(TargetServiceContractRule::Authority));
    }
    let declaration = graph
        .declarations()
        .callables()
        .get(callable)
        .ok_or_else(|| fail(TargetServiceContractRule::Authority))?;
    let CallableOwner::Module(owner_module) = declaration.owner() else {
        return Err(fail(TargetServiceContractRule::Authority));
    };
    let module = graph
        .modules()
        .get(owner_module)
        .ok_or_else(|| fail(TargetServiceContractRule::Authority))?;
    if module.package() != standard_package || declaration.name().is_none() {
        return Err(fail(TargetServiceContractRule::Authority));
    }
    if declaration.kind() != CallableKind::Primitive || declaration.receiver().is_some() {
        return Err(fail(TargetServiceContractRule::CallableKind));
    }
    let site = graph
        .declaration_sites()
        .get(declaration.site())
        .ok_or_else(|| fail(TargetServiceContractRule::Authority))?;
    if site.module() != owner_module || site.visibility() != Visibility::Private {
        return Err(fail(TargetServiceContractRule::Visibility));
    }
    if !declaration.generic_parameters().is_empty() {
        return Err(fail(TargetServiceContractRule::GenericShape));
    }
    if !declaration.requirements().is_empty() {
        return Err(fail(TargetServiceContractRule::Requirements));
    }
    if declaration.body().is_some() {
        return Err(fail(TargetServiceContractRule::Body));
    }
    if declaration.guarantees().allocation() != AllocationGuarantee::NoAllocation {
        return Err(fail(TargetServiceContractRule::Guarantees));
    }
    if declaration.target_gate() != Some(descriptor.target()) {
        return Err(fail(TargetServiceContractRule::TargetGate));
    }

    let signature = descriptor.signature();
    if declaration.parameters().len() != signature.parameters().len() {
        return Err(fail(TargetServiceContractRule::ParameterShape));
    }
    for (position, (parameter, expected)) in declaration
        .parameters()
        .iter()
        .zip(signature.parameters())
        .enumerate()
    {
        let parameter = graph
            .declarations()
            .parameters()
            .get(*parameter)
            .ok_or_else(|| fail(TargetServiceContractRule::ParameterShape))?;
        if parameter.owner() != ParameterOwner::Callable(callable)
            || parameter.role() != (ParameterRole::Ordinary { position })
            || !type_matches(types, parameter.ty(), *expected)
        {
            return Err(fail(TargetServiceContractRule::ParameterShape));
        }
    }
    let result_matches = match signature.result() {
        Some(expected) => type_matches(types, declaration.result(), expected),
        None => types.get(declaration.result()) == Some(&TypeKind::Builtin(BuiltinType::Void)),
    };
    if !result_matches {
        return Err(fail(TargetServiceContractRule::ResultType));
    }
    Ok(())
}

fn type_matches(types: &TypeStore, ty: TypeId, expected: TargetServiceValueAbi) -> bool {
    match expected {
        TargetServiceValueAbi::CInt => types.get(ty) == Some(&TypeKind::Builtin(BuiltinType::I32)),
        TargetServiceValueAbi::Word => {
            types.get(ty) == Some(&TypeKind::Builtin(BuiltinType::Usize))
        }
        TargetServiceValueAbi::Pointer => matches!(types.get(ty), Some(TypeKind::Pointer(_))),
    }
}
