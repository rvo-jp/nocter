use std::collections::BTreeMap;
use std::fmt;

use nocter_declaration_lowering::StandardRoleInput;
use nocter_declarations::{
    CallableKind, CallableOwner, DeclarationGraph, NominalShape, ParameterRole,
    StandardDeclarationRole, Visibility,
};
use nocter_model::{
    AssociatedTypeId, BorrowCapability, BuiltinType, CallableCapability, CallableId,
    DeclarationSiteId, InterfaceId, NominalTypeId, TypeKind, TypeStore,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole, SyntaxOrigin};

mod interpolation;

use interpolation::validate_interpolation_roles;

/// Exact standard declarations selected by toolchain discovery and validated once for Phase 3.
///
/// Consumers query semantic roles, never source spellings. A table may omit roles that the active
/// compile unit does not need; a feature requiring one reports that missing capability at its own
/// checked boundary.
#[derive(Debug, Default)]
pub struct StandardSemanticTable {
    entries: BTreeMap<StandardDeclarationRole, SemanticEntity>,
}

impl StandardSemanticTable {
    pub(crate) fn build(
        inputs: &[StandardRoleInput],
        graph: &DeclarationGraph,
        types: &TypeStore,
        source_index: &SourceIndex,
    ) -> Result<Self, StandardSemanticError> {
        let mut inputs = inputs.to_vec();
        inputs.sort_by_key(|input| input.role());
        for duplicate in inputs.windows(2) {
            if duplicate[0].role() == duplicate[1].role() {
                return Err(StandardSemanticError::DuplicateRole(duplicate[0].role()));
            }
        }
        let mut entries = BTreeMap::new();
        for input in inputs {
            let entity = resolve_role_source(input, source_index)?;
            validate_role_domain(input.role(), entity)?;
            validate_standard_owner(graph, input.role(), entity)?;
            entries.insert(input.role(), entity);
        }
        let table = Self { entries };
        table.validate_nominal_roles(graph, types)?;
        table.validate_relationships(graph, types)?;
        Ok(table)
    }

    #[must_use]
    pub fn nominal(&self, role: StandardDeclarationRole) -> Option<NominalTypeId> {
        match self.entries.get(&role) {
            Some(SemanticEntity::NominalType(id)) => Some(*id),
            _ => None,
        }
    }

    #[must_use]
    pub fn interface(&self, role: StandardDeclarationRole) -> Option<InterfaceId> {
        match self.entries.get(&role) {
            Some(SemanticEntity::Interface(id)) => Some(*id),
            _ => None,
        }
    }

    #[must_use]
    pub fn callable(&self, role: StandardDeclarationRole) -> Option<CallableId> {
        match self.entries.get(&role) {
            Some(SemanticEntity::Callable(id)) => Some(*id),
            _ => None,
        }
    }

    #[must_use]
    pub fn associated_type(&self, role: StandardDeclarationRole) -> Option<AssociatedTypeId> {
        match self.entries.get(&role) {
            Some(SemanticEntity::AssociatedType(id)) => Some(*id),
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
        self.validate_iteration_relationships(graph, types)?;
        self.validate_exact_size_relationships(graph, types)
    }

    fn validate_iteration_relationships(
        &self,
        graph: &DeclarationGraph,
        types: &TypeStore,
    ) -> Result<(), StandardSemanticError> {
        let item = self.associated_type(StandardDeclarationRole::IteratorItem);
        let next = self.callable(StandardDeclarationRole::IteratorNextMethod);
        if item.is_none() && next.is_none() {
            return Ok(());
        }
        let interface = self
            .interface(StandardDeclarationRole::IteratorInterface)
            .ok_or(StandardSemanticError::MissingDependency {
                role: if item.is_some() {
                    StandardDeclarationRole::IteratorItem
                } else {
                    StandardDeclarationRole::IteratorNextMethod
                },
                dependency: StandardDeclarationRole::IteratorInterface,
            })?;
        let item = item.ok_or(StandardSemanticError::MissingDependency {
            role: StandardDeclarationRole::IteratorNextMethod,
            dependency: StandardDeclarationRole::IteratorItem,
        })?;
        validate_iterator_item(graph, interface, item)?;
        if let Some(next) = next {
            validate_iterator_next(graph, types, interface, item, next)?;
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

fn resolve_role_source(
    input: StandardRoleInput,
    source_index: &SourceIndex,
) -> Result<SemanticEntity, StandardSemanticError> {
    let token = input.declaration();
    let mut matches = source_index
        .bindings_at(token.source(), token.range().start())
        .filter(|binding| {
            binding.role() == SourceRole::Declaration
                && binding.origin().syntax() == SyntaxOrigin::Token(token)
        })
        .map(|binding| binding.entity());
    let Some(entity) = matches.next() else {
        return Err(StandardSemanticError::MissingDeclaration(input.role()));
    };
    if matches.next().is_some() {
        return Err(StandardSemanticError::AmbiguousDeclaration(input.role()));
    }
    Ok(entity)
}

fn validate_role_domain(
    role: StandardDeclarationRole,
    entity: SemanticEntity,
) -> Result<(), StandardSemanticError> {
    let valid = match role {
        StandardDeclarationRole::AbortingAllocator
        | StandardDeclarationRole::AllocationContext
        | StandardDeclarationRole::OwnedString => matches!(entity, SemanticEntity::NominalType(_)),
        StandardDeclarationRole::FormatInterface
        | StandardDeclarationRole::IteratorInterface
        | StandardDeclarationRole::ExactSizeIteratorInterface => {
            matches!(entity, SemanticEntity::Interface(_))
        }
        StandardDeclarationRole::IteratorItem => {
            matches!(entity, SemanticEntity::AssociatedType(_))
        }
        StandardDeclarationRole::FormatMethod
        | StandardDeclarationRole::InterpolationConstructor
        | StandardDeclarationRole::InterpolationTextAppender
        | StandardDeclarationRole::IteratorNextMethod
        | StandardDeclarationRole::ExactSizeIteratorRemainingLenMethod => {
            matches!(entity, SemanticEntity::Callable(_))
        }
    };
    if valid {
        Ok(())
    } else {
        Err(StandardSemanticError::WrongDeclarationKind(role))
    }
}

fn validate_standard_owner(
    graph: &DeclarationGraph,
    role: StandardDeclarationRole,
    entity: SemanticEntity,
) -> Result<(), StandardSemanticError> {
    let standard = graph
        .standard_library()
        .ok_or(StandardSemanticError::MissingStandardPackage)?;
    let site = entity_site(graph, entity).ok_or(StandardSemanticError::MissingSite(role))?;
    let module = graph
        .declaration_sites()
        .get(site)
        .and_then(|site| graph.modules().get(site.module()))
        .ok_or(StandardSemanticError::MissingSite(role))?;
    if module.package() != standard.package() {
        return Err(StandardSemanticError::OutsideStandardPackage(role));
    }
    Ok(())
}

fn entity_site(graph: &DeclarationGraph, entity: SemanticEntity) -> Option<DeclarationSiteId> {
    match entity {
        SemanticEntity::NominalType(id) => graph
            .declarations()
            .nominal_types()
            .get(id)
            .map(nocter_declarations::NominalTypeDeclaration::site),
        SemanticEntity::Interface(id) => graph
            .declarations()
            .interfaces()
            .get(id)
            .map(nocter_declarations::InterfaceDeclaration::site),
        SemanticEntity::AssociatedType(id) => graph
            .declarations()
            .associated_types()
            .get(id)
            .map(nocter_declarations::AssociatedTypeDeclaration::site),
        SemanticEntity::Callable(id) => graph
            .declarations()
            .callables()
            .get(id)
            .map(nocter_declarations::CallableDeclaration::site),
        _ => None,
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
        || callable.body().is_some()
        || !is_public(graph, string_declaration.site())
        || !is_public(graph, interface_declaration.site())
        || !is_public(graph, callable.site())
        || receiver.role() != ParameterRole::Receiver(CallableCapability::Readonly)
        || output.role()
            != (ParameterRole::Ordinary {
                position: 0,
                variadic: false,
            })
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
) -> Result<(), StandardSemanticError> {
    let interface_declaration = graph
        .declarations()
        .interfaces()
        .get(interface)
        .ok_or(StandardSemanticError::InvalidIteratorContract)?;
    let item_declaration = graph
        .declarations()
        .associated_types()
        .get(item)
        .ok_or(StandardSemanticError::InvalidIteratorContract)?;
    if !interface_declaration.generic_parameters().is_empty()
        || !interface_declaration.associated_types().contains(&item)
        || item_declaration.interface() != interface
        || !item_declaration.bounds().is_empty()
        || !is_public(graph, interface_declaration.site())
        || !is_public(graph, item_declaration.site())
    {
        return Err(StandardSemanticError::InvalidIteratorContract);
    }
    Ok(())
}

fn validate_iterator_next(
    graph: &DeclarationGraph,
    types: &TypeStore,
    interface: InterfaceId,
    item: AssociatedTypeId,
    method: CallableId,
) -> Result<(), StandardSemanticError> {
    let interface_declaration = graph
        .declarations()
        .interfaces()
        .get(interface)
        .ok_or(StandardSemanticError::InvalidIteratorContract)?;
    let callable = graph
        .declarations()
        .callables()
        .get(method)
        .ok_or(StandardSemanticError::InvalidIteratorContract)?;
    let Some(receiver) = callable
        .receiver()
        .and_then(|id| graph.declarations().parameters().get(id))
    else {
        return Err(StandardSemanticError::InvalidIteratorContract);
    };
    let Some(TypeKind::Optional(result)) = types.get(callable.result()) else {
        return Err(StandardSemanticError::InvalidIteratorContract);
    };
    let Some(TypeKind::AssociatedProjection { base, associated }) = types.get(*result) else {
        return Err(StandardSemanticError::InvalidIteratorContract);
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
        return Err(StandardSemanticError::InvalidIteratorContract);
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

fn is_public(graph: &DeclarationGraph, site: DeclarationSiteId) -> bool {
    graph
        .declaration_sites()
        .get(site)
        .is_some_and(|site| site.visibility() == Visibility::Public)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardSemanticError {
    DuplicateRole(StandardDeclarationRole),
    MissingDeclaration(StandardDeclarationRole),
    AmbiguousDeclaration(StandardDeclarationRole),
    WrongDeclarationKind(StandardDeclarationRole),
    MissingStandardPackage,
    MissingSite(StandardDeclarationRole),
    OutsideStandardPackage(StandardDeclarationRole),
    MissingDependency {
        role: StandardDeclarationRole,
        dependency: StandardDeclarationRole,
    },
    InvalidFormatContract,
    InvalidInterpolationContract,
    InvalidIteratorContract,
    InvalidExactSizeIteratorContract,
    InvalidNominalContract(StandardDeclarationRole),
}

impl fmt::Display for StandardSemanticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid standard semantic role input: {self:?}")
    }
}

impl std::error::Error for StandardSemanticError {}
