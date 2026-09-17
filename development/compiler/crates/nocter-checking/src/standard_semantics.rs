use std::collections::BTreeMap;
use std::fmt;

use nocter_declarations::{
    CallableExecution, CallableKind, CallableOwner, CallableProvenanceContract, DeclarationGraph,
    NominalShape, ParameterRole, ProvenanceAnnotation, ProvenanceOrigin, StandardDeclaration,
    Visibility,
};
use nocter_model::{
    AssociatedTypeId, BorrowCapability, BuiltinType, CallableCapability, CallableId,
    DeclarationSiteId, InterfaceId, NominalTypeId, TypeKind, TypeStore,
};
use nocter_toolchain_contract::StandardDeclarationRole;

mod interpolation;

use interpolation::validate_interpolation_roles;

#[derive(Clone, Copy)]
enum IterationResultContract {
    ImmediateOptional,
    DeferredFallibleOptional,
}

#[derive(Clone, Copy)]
struct IterationSemanticRoles {
    interface: StandardDeclarationRole,
    item: StandardDeclarationRole,
    next: StandardDeclarationRole,
    result: IterationResultContract,
    lends_from_receiver: bool,
}

impl IterationSemanticRoles {
    const SYNCHRONOUS: Self = Self {
        interface: StandardDeclarationRole::IteratorInterface,
        item: StandardDeclarationRole::IteratorItem,
        next: StandardDeclarationRole::IteratorNextMethod,
        result: IterationResultContract::ImmediateOptional,
        lends_from_receiver: false,
    };

    const LENDING: Self = Self {
        interface: StandardDeclarationRole::LendingIteratorInterface,
        item: StandardDeclarationRole::LendingIteratorItem,
        next: StandardDeclarationRole::LendingIteratorNextMethod,
        result: IterationResultContract::ImmediateOptional,
        lends_from_receiver: true,
    };

    const ASYNCHRONOUS: Self = Self {
        interface: StandardDeclarationRole::AsyncIteratorInterface,
        item: StandardDeclarationRole::AsyncIteratorItem,
        next: StandardDeclarationRole::AsyncIteratorNextMethod,
        result: IterationResultContract::DeferredFallibleOptional,
        lends_from_receiver: false,
    };

    const ASYNCHRONOUS_LENDING: Self = Self {
        interface: StandardDeclarationRole::AsyncLendingIteratorInterface,
        item: StandardDeclarationRole::AsyncLendingIteratorItem,
        next: StandardDeclarationRole::AsyncLendingIteratorNextMethod,
        result: IterationResultContract::DeferredFallibleOptional,
        lends_from_receiver: true,
    };

    const fn invalid(self) -> StandardSemanticError {
        match self.result {
            IterationResultContract::ImmediateOptional if self.lends_from_receiver => {
                StandardSemanticError::InvalidLendingIteratorContract
            }
            IterationResultContract::ImmediateOptional => {
                StandardSemanticError::InvalidIteratorContract
            }
            IterationResultContract::DeferredFallibleOptional if self.lends_from_receiver => {
                StandardSemanticError::InvalidAsyncLendingIteratorContract
            }
            IterationResultContract::DeferredFallibleOptional => {
                StandardSemanticError::InvalidAsyncIteratorContract
            }
        }
    }
}

/// Exact standard declarations selected by toolchain discovery and validated once for Phase 3.
///
/// Consumers query semantic roles, never source spellings. A table may omit roles that the active
/// compile unit does not need; a feature requiring one reports that missing capability at its own
/// checked boundary.
#[derive(Clone, Debug, Default)]
pub struct StandardSemanticTable {
    entries: BTreeMap<StandardDeclarationRole, StandardDeclaration>,
}

impl StandardSemanticTable {
    pub(crate) fn build(
        graph: &DeclarationGraph,
        types: &TypeStore,
    ) -> Result<Self, StandardSemanticError> {
        let standard = graph
            .standard_library()
            .ok_or(StandardSemanticError::MissingStandardPackage)?;
        let mut entries = BTreeMap::new();
        for (role, declaration) in standard.declarations() {
            validate_role_domain(role, declaration)?;
            entries.insert(role, declaration);
        }
        let table = Self { entries };
        table.validate_nominal_roles(graph, types)?;
        table.validate_relationships(graph, types)?;
        Ok(table)
    }

    #[must_use]
    pub fn nominal(&self, role: StandardDeclarationRole) -> Option<NominalTypeId> {
        match self.entries.get(&role) {
            Some(StandardDeclaration::NominalType(id)) => Some(*id),
            _ => None,
        }
    }

    #[must_use]
    pub fn interface(&self, role: StandardDeclarationRole) -> Option<InterfaceId> {
        match self.entries.get(&role) {
            Some(StandardDeclaration::Interface(id)) => Some(*id),
            _ => None,
        }
    }

    #[must_use]
    pub fn callable(&self, role: StandardDeclarationRole) -> Option<CallableId> {
        match self.entries.get(&role) {
            Some(StandardDeclaration::Callable(id)) => Some(*id),
            _ => None,
        }
    }

    #[must_use]
    pub fn associated_type(&self, role: StandardDeclarationRole) -> Option<AssociatedTypeId> {
        match self.entries.get(&role) {
            Some(StandardDeclaration::AssociatedType(id)) => Some(*id),
            _ => None,
        }
    }

    fn validate_relationships(
        &self,
        graph: &DeclarationGraph,
        types: &TypeStore,
    ) -> Result<(), StandardSemanticError> {
        if let Some(method) = self.callable(StandardDeclarationRole::FormatMethod) {
            let interface = self
                .interface(StandardDeclarationRole::FormatInterface)
                .ok_or(StandardSemanticError::MissingDependency {
                    role: StandardDeclarationRole::FormatMethod,
                    dependency: StandardDeclarationRole::FormatInterface,
                })?;
            let string = self.nominal(StandardDeclarationRole::OwnedString).ok_or(
                StandardSemanticError::MissingDependency {
                    role: StandardDeclarationRole::FormatMethod,
                    dependency: StandardDeclarationRole::OwnedString,
                },
            )?;
            validate_format_method(graph, types, interface, string, method)?;
        }
        validate_interpolation_roles(
            graph,
            types,
            self.nominal(StandardDeclarationRole::OwnedString),
            self.callable(StandardDeclarationRole::InterpolationConstructor),
            self.callable(StandardDeclarationRole::InterpolationTextAppender),
        )?;
        if let Some(abort) = self.callable(StandardDeclarationRole::ProcessAbort) {
            validate_process_abort(graph, types, abort)?;
        }
        self.validate_iteration_relationships(graph, types, IterationSemanticRoles::SYNCHRONOUS)?;
        self.validate_iteration_relationships(graph, types, IterationSemanticRoles::LENDING)?;
        self.validate_iteration_relationships(graph, types, IterationSemanticRoles::ASYNCHRONOUS)?;
        self.validate_iteration_relationships(
            graph,
            types,
            IterationSemanticRoles::ASYNCHRONOUS_LENDING,
        )?;
        self.validate_exact_size_relationships(graph, types)
    }

    fn validate_iteration_relationships(
        &self,
        graph: &DeclarationGraph,
        types: &TypeStore,
        roles: IterationSemanticRoles,
    ) -> Result<(), StandardSemanticError> {
        let item = self.associated_type(roles.item);
        let next = self.callable(roles.next);
        if item.is_none() && next.is_none() {
            return Ok(());
        }
        let interface =
            self.interface(roles.interface)
                .ok_or(StandardSemanticError::MissingDependency {
                    role: if item.is_some() {
                        roles.item
                    } else {
                        roles.next
                    },
                    dependency: roles.interface,
                })?;
        let item = item.ok_or(StandardSemanticError::MissingDependency {
            role: roles.next,
            dependency: roles.item,
        })?;
        validate_iterator_item(graph, interface, item, roles)?;
        if let Some(next) = next {
            validate_iterator_next(graph, types, interface, item, next, roles)?;
        }
        Ok(())
    }

    fn validate_exact_size_relationships(
        &self,
        graph: &DeclarationGraph,
        types: &TypeStore,
    ) -> Result<(), StandardSemanticError> {
        let Some(method) =
            self.callable(StandardDeclarationRole::ExactSizeIteratorRemainingLenMethod)
        else {
            return Ok(());
        };
        let interface = self
            .interface(StandardDeclarationRole::ExactSizeIteratorInterface)
            .ok_or(StandardSemanticError::MissingDependency {
                role: StandardDeclarationRole::ExactSizeIteratorRemainingLenMethod,
                dependency: StandardDeclarationRole::ExactSizeIteratorInterface,
            })?;
        validate_exact_size_method(graph, types, interface, method)
    }

    fn validate_nominal_roles(
        &self,
        graph: &DeclarationGraph,
        types: &TypeStore,
    ) -> Result<(), StandardSemanticError> {
        for role in [
            StandardDeclarationRole::AbortingAllocator,
            StandardDeclarationRole::AllocationContext,
            StandardDeclarationRole::OwnedString,
        ] {
            let Some(nominal) = self.nominal(role) else {
                continue;
            };
            let Some(declaration) = graph.declarations().nominal_types().get(nominal) else {
                return Err(StandardSemanticError::InvalidNominalContract(role));
            };
            if !declaration.generic_parameters().is_empty() || !is_public(graph, declaration.site())
            {
                return Err(StandardSemanticError::InvalidNominalContract(role));
            }
            if matches!(
                role,
                StandardDeclarationRole::AbortingAllocator
                    | StandardDeclarationRole::AllocationContext
            ) && !has_allocation_context_header(graph, types, declaration)
            {
                return Err(StandardSemanticError::InvalidNominalContract(role));
            }
        }
        Ok(())
    }
}

fn has_allocation_context_header(
    graph: &DeclarationGraph,
    types: &TypeStore,
    declaration: &nocter_declarations::NominalTypeDeclaration,
) -> bool {
    let NominalShape::Struct { fields, .. } = declaration.shape() else {
        return false;
    };
    fields.get(..2).is_some_and(|header| {
        header.iter().all(|field| {
            graph
                .declarations()
                .fields()
                .get(*field)
                .is_some_and(|field| field.ty() == types.builtin(BuiltinType::Usize))
        })
    })
}

fn validate_role_domain(
    role: StandardDeclarationRole,
    entity: StandardDeclaration,
) -> Result<(), StandardSemanticError> {
    let valid = match role {
        StandardDeclarationRole::AbortingAllocator
        | StandardDeclarationRole::AllocationContext
        | StandardDeclarationRole::OwnedString => {
            matches!(entity, StandardDeclaration::NominalType(_))
        }
        StandardDeclarationRole::FormatInterface
        | StandardDeclarationRole::IteratorInterface
        | StandardDeclarationRole::LendingIteratorInterface
        | StandardDeclarationRole::AsyncIteratorInterface
        | StandardDeclarationRole::AsyncLendingIteratorInterface
        | StandardDeclarationRole::ExactSizeIteratorInterface => {
            matches!(entity, StandardDeclaration::Interface(_))
        }
        StandardDeclarationRole::IteratorItem
        | StandardDeclarationRole::LendingIteratorItem
        | StandardDeclarationRole::AsyncIteratorItem
        | StandardDeclarationRole::AsyncLendingIteratorItem => {
            matches!(entity, StandardDeclaration::AssociatedType(_))
        }
        StandardDeclarationRole::FormatMethod
        | StandardDeclarationRole::AllocationRequest
        | StandardDeclarationRole::InterpolationConstructor
        | StandardDeclarationRole::InterpolationTextAppender
        | StandardDeclarationRole::IteratorNextMethod
        | StandardDeclarationRole::LendingIteratorNextMethod
        | StandardDeclarationRole::AsyncIteratorNextMethod
        | StandardDeclarationRole::AsyncLendingIteratorNextMethod
        | StandardDeclarationRole::ExactSizeIteratorRemainingLenMethod
        | StandardDeclarationRole::ProcessAbort => {
            matches!(entity, StandardDeclaration::Callable(_))
        }
    };
    if valid {
        Ok(())
    } else {
        Err(StandardSemanticError::WrongDeclarationKind(role))
    }
}

fn validate_format_method(
    graph: &DeclarationGraph,
    types: &TypeStore,
    interface: InterfaceId,
    string: NominalTypeId,
    method: CallableId,
) -> Result<(), StandardSemanticError> {
    let interface_declaration = graph
        .declarations()
        .interfaces()
        .get(interface)
        .ok_or(StandardSemanticError::InvalidFormatContract)?;
    let string_declaration = graph
        .declarations()
        .nominal_types()
        .get(string)
        .ok_or(StandardSemanticError::InvalidFormatContract)?;
    let callable = graph
        .declarations()
        .callables()
        .get(method)
        .ok_or(StandardSemanticError::InvalidFormatContract)?;
    let Some(receiver) = callable
        .receiver()
        .and_then(|id| graph.declarations().parameters().get(id))
    else {
        return Err(StandardSemanticError::InvalidFormatContract);
    };
    let [output] = callable.parameters() else {
        return Err(StandardSemanticError::InvalidFormatContract);
    };
    let output = graph
        .declarations()
        .parameters()
        .get(*output)
        .ok_or(StandardSemanticError::InvalidFormatContract)?;
    if !interface_declaration.generic_parameters().is_empty()
        || !interface_declaration.associated_types().is_empty()
        || !string_declaration.generic_parameters().is_empty()
        || callable.kind() != CallableKind::Method
        || callable.owner() != CallableOwner::Interface(interface)
        || !interface_declaration.methods().contains(&method)
        || !callable.generic_parameters().is_empty()
        || !callable.requirements().is_empty()
        || callable.body().is_none()
        || !is_public(graph, string_declaration.site())
        || !is_public(graph, interface_declaration.site())
        || !is_public(graph, callable.site())
        || receiver.role() != ParameterRole::Receiver(CallableCapability::Readonly)
        || output.role() != (ParameterRole::Ordinary { position: 0 })
        || callable.result() != types.builtin(BuiltinType::Void)
    {
        return Err(StandardSemanticError::InvalidFormatContract);
    }
    let Some(TypeKind::InterfaceSelf(receiver_interface)) = types.get(receiver.ty()) else {
        return Err(StandardSemanticError::InvalidFormatContract);
    };
    let Some(TypeKind::Borrow {
        capability: BorrowCapability::ReadWrite,
        referent,
    }) = types.get(output.ty())
    else {
        return Err(StandardSemanticError::InvalidFormatContract);
    };
    let Some(TypeKind::Nominal {
        definition,
        arguments,
    }) = types.get(*referent)
    else {
        return Err(StandardSemanticError::InvalidFormatContract);
    };
    if *receiver_interface != interface || *definition != string || !arguments.is_empty() {
        return Err(StandardSemanticError::InvalidFormatContract);
    }
    Ok(())
}

fn validate_iterator_item(
    graph: &DeclarationGraph,
    interface: InterfaceId,
    item: AssociatedTypeId,
    roles: IterationSemanticRoles,
) -> Result<(), StandardSemanticError> {
    let invalid = roles.invalid();
    let interface_declaration = graph
        .declarations()
        .interfaces()
        .get(interface)
        .ok_or(invalid)?;
    let item_declaration = graph
        .declarations()
        .associated_types()
        .get(item)
        .ok_or(invalid)?;
    if !interface_declaration.generic_parameters().is_empty()
        || !interface_declaration.associated_types().contains(&item)
        || item_declaration.interface() != interface
        || !item_declaration.bounds().is_empty()
        || !is_public(graph, interface_declaration.site())
        || !is_public(graph, item_declaration.site())
    {
        return Err(invalid);
    }
    Ok(())
}

fn validate_iterator_next(
    graph: &DeclarationGraph,
    types: &TypeStore,
    interface: InterfaceId,
    item: AssociatedTypeId,
    method: CallableId,
    roles: IterationSemanticRoles,
) -> Result<(), StandardSemanticError> {
    let invalid = roles.invalid();
    let interface_declaration = graph
        .declarations()
        .interfaces()
        .get(interface)
        .ok_or(invalid)?;
    let callable = graph
        .declarations()
        .callables()
        .get(method)
        .ok_or(invalid)?;
    let Some(receiver) = callable
        .receiver()
        .and_then(|id| graph.declarations().parameters().get(id))
    else {
        return Err(invalid);
    };
    let result = match roles.result {
        IterationResultContract::ImmediateOptional => {
            let Some(TypeKind::Optional(result)) = types.get(callable.body_result()) else {
                return Err(invalid);
            };
            if callable.execution() != CallableExecution::Immediate {
                return Err(invalid);
            }
            *result
        }
        IterationResultContract::DeferredFallibleOptional => {
            let Some(TypeKind::Fallible(optional)) = types.get(callable.body_result()) else {
                return Err(invalid);
            };
            let Some(TypeKind::Optional(result)) = types.get(*optional) else {
                return Err(invalid);
            };
            if !matches!(callable.execution(), CallableExecution::Deferred { .. }) {
                return Err(invalid);
            }
            *result
        }
    };
    let Some(TypeKind::AssociatedProjection { base, associated }) = types.get(result) else {
        return Err(invalid);
    };
    if callable.kind() != CallableKind::Method
        || callable.owner() != CallableOwner::Interface(interface)
        || !interface_declaration.methods().contains(&method)
        || receiver.role() != ParameterRole::Receiver(CallableCapability::ReadWrite)
        || !callable.parameters().is_empty()
        || !callable.generic_parameters().is_empty()
        || !callable.requirements().is_empty()
        || callable.body().is_some()
        || !matches!(types.get(*base), Some(TypeKind::InterfaceSelf(id)) if *id == interface)
        || *associated != item
        || !is_public(graph, callable.site())
    {
        return Err(invalid);
    }
    if roles.lends_from_receiver
        && (!matches!(
            callable.provenance(),
            CallableProvenanceContract::Declared(provenance)
                if provenance.origins() == [ProvenanceOrigin::Receiver]
        ) || !matches!(
            callable.provenance_annotation(),
            ProvenanceAnnotation::Explicit { .. }
        ))
    {
        return Err(invalid);
    }
    Ok(())
}

fn validate_exact_size_method(
    graph: &DeclarationGraph,
    types: &TypeStore,
    interface: InterfaceId,
    method: CallableId,
) -> Result<(), StandardSemanticError> {
    let interface_declaration = graph
        .declarations()
        .interfaces()
        .get(interface)
        .ok_or(StandardSemanticError::InvalidExactSizeIteratorContract)?;
    let callable = graph
        .declarations()
        .callables()
        .get(method)
        .ok_or(StandardSemanticError::InvalidExactSizeIteratorContract)?;
    let Some(receiver) = callable
        .receiver()
        .and_then(|id| graph.declarations().parameters().get(id))
    else {
        return Err(StandardSemanticError::InvalidExactSizeIteratorContract);
    };
    if !interface_declaration.generic_parameters().is_empty()
        || !interface_declaration.associated_types().is_empty()
        || callable.kind() != CallableKind::Method
        || callable.owner() != CallableOwner::Interface(interface)
        || !interface_declaration.methods().contains(&method)
        || receiver.role() != ParameterRole::Receiver(CallableCapability::Readonly)
        || !callable.parameters().is_empty()
        || !callable.generic_parameters().is_empty()
        || !callable.requirements().is_empty()
        || callable.result() != types.builtin(BuiltinType::Usize)
        || callable.body().is_some()
        || !is_public(graph, interface_declaration.site())
        || !is_public(graph, callable.site())
    {
        return Err(StandardSemanticError::InvalidExactSizeIteratorContract);
    }
    Ok(())
}

fn validate_process_abort(
    graph: &DeclarationGraph,
    types: &TypeStore,
    abort: CallableId,
) -> Result<(), StandardSemanticError> {
    let callable = graph
        .declarations()
        .callables()
        .get(abort)
        .ok_or(StandardSemanticError::InvalidProcessAbortContract)?;
    if callable.kind() != CallableKind::Function
        || !matches!(callable.owner(), CallableOwner::Module(_))
        || callable.receiver().is_some()
        || !callable.parameters().is_empty()
        || !callable.generic_parameters().is_empty()
        || !callable.requirements().is_empty()
        || callable.result() != types.builtin(BuiltinType::Never)
        || !is_public(graph, callable.site())
    {
        return Err(StandardSemanticError::InvalidProcessAbortContract);
    }
    Ok(())
}

fn is_public(graph: &DeclarationGraph, site: DeclarationSiteId) -> bool {
    graph
        .declaration_sites()
        .get(site)
        .is_some_and(|site| site.visibility() == Visibility::Public)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardSemanticError {
    WrongDeclarationKind(StandardDeclarationRole),
    MissingStandardPackage,
    MissingDependency {
        role: StandardDeclarationRole,
        dependency: StandardDeclarationRole,
    },
    InvalidFormatContract,
    InvalidInterpolationContract,
    InvalidIteratorContract,
    InvalidLendingIteratorContract,
    InvalidAsyncIteratorContract,
    InvalidAsyncLendingIteratorContract,
    InvalidExactSizeIteratorContract,
    InvalidProcessAbortContract,
    InvalidNominalContract(StandardDeclarationRole),
}

impl fmt::Display for StandardSemanticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid standard semantic role input: {self:?}")
    }
}

impl std::error::Error for StandardSemanticError {}
