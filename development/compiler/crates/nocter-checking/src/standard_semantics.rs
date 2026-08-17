use std::collections::BTreeMap;
use std::fmt;

use nocter_declaration_lowering::StandardRoleInput;
use nocter_declarations::{
    CallableKind, CallableOwner, DeclarationGraph, ParameterRole, StandardDeclarationRole,
};
use nocter_model::{
    BorrowCapability, BuiltinType, CallableCapability, CallableId, DeclarationSiteId, InterfaceId,
    NominalTypeId, TypeKind, TypeStore,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole, SyntaxOrigin};

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
        table.validate_nominal_roles(graph)?;
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

    fn validate_relationships(
        &self,
        graph: &DeclarationGraph,
        types: &TypeStore,
    ) -> Result<(), StandardSemanticError> {
        let Some(method) = self.callable(StandardDeclarationRole::FormatMethod) else {
            return Ok(());
        };
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
        validate_format_method(graph, types, interface, string, method)
    }

    fn validate_nominal_roles(
        &self,
        graph: &DeclarationGraph,
    ) -> Result<(), StandardSemanticError> {
        for role in [
            StandardDeclarationRole::AbortingAllocator,
            StandardDeclarationRole::AllocationContext,
            StandardDeclarationRole::OwnedString,
        ] {
            let Some(nominal) = self.nominal(role) else {
                continue;
            };
            if !graph
                .declarations()
                .nominal_types()
                .get(nominal)
                .is_some_and(|declaration| declaration.generic_parameters().is_empty())
            {
                return Err(StandardSemanticError::InvalidNominalContract(role));
            }
        }
        Ok(())
    }
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
        StandardDeclarationRole::FormatInterface => matches!(entity, SemanticEntity::Interface(_)),
        StandardDeclarationRole::FormatMethod => matches!(entity, SemanticEntity::Callable(_)),
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
        || !string_declaration.generic_parameters().is_empty()
        || callable.kind() != CallableKind::Method
        || callable.owner() != CallableOwner::Interface(interface)
        || !interface_declaration.methods().contains(&method)
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
    InvalidNominalContract(StandardDeclarationRole),
}

impl fmt::Display for StandardSemanticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid standard semantic role input: {self:?}")
    }
}

impl std::error::Error for StandardSemanticError {}
