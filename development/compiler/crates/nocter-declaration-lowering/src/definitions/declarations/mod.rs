use std::collections::{HashMap, HashSet};

use nocter_declarations::{
    AssociatedTypeBinding, AssociatedTypeDeclaration, ConformanceDeclaration,
    ConstructionDeclaration, DropDeclaration, InstanceDeclaration, InterfaceApplication,
    InterfaceDeclaration, NominalShape, NominalTypeDeclaration, OpaqueTypeDeclaration,
    TestDeclaration, TypeAliasDeclaration, VariantDeclaration,
};
use nocter_model::{AssociatedTypeId, CallableId, InterfaceId};
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole, SyntaxOrigin};
use nocter_syntax::{NodeKind, TokenKind};

use crate::{
    NormalizedDeclarationPattern, PreparedTypes, ReservedEntity, SurfaceDeclarationId,
    SurfaceDeclarationKind,
};

use super::allocation::{
    AllocatedHeaders, entity, name, pattern_type, representative, site, surface_count,
    surface_kind, surface_node, surface_owner,
};
use super::{DefinitionRule, DefinitionViolation, HeaderDefinitionError, projection, syntax};

mod callable;
mod constant;
mod provenance;
mod target;

pub(super) fn define(
    types: &mut PreparedTypes<'_>,
    allocated: &mut AllocatedHeaders,
) -> Result<(), HeaderDefinitionError> {
    constant::define_all(types)?;
    for index in 0..surface_count(types) {
        let declaration = SurfaceDeclarationId::from_index(index);
        if representative(types, declaration) != declaration {
            continue;
        }
        match entity(types, declaration) {
            Some(ReservedEntity::NominalType(id)) => {
                define_nominal(types, allocated, declaration, id)?;
            }
            Some(ReservedEntity::TypeAlias(id)) => define_alias(types, allocated, declaration, id)?,
            Some(ReservedEntity::Interface(id)) => {
                define_interface(types, allocated, declaration, id)?;
            }
            Some(ReservedEntity::AssociatedType(id)) => {
                define_associated(types, allocated, declaration, id)?;
            }
            Some(ReservedEntity::Constant(_)) => {}
            Some(ReservedEntity::Callable(id)) => {
                callable::define(types, allocated, declaration, id)?;
            }
            Some(ReservedEntity::Construction(id)) => {
                define_construction(types, allocated, declaration, id)?;
            }
            Some(ReservedEntity::Instance(id)) => {
                define_instance(types, allocated, declaration, id)?;
            }
            Some(ReservedEntity::Conformance(id)) => {
                define_conformance(types, allocated, declaration, id)?;
            }
            Some(ReservedEntity::Drop(id)) => define_drop(types, allocated, declaration, id)?,
            Some(ReservedEntity::Test(id)) => define_test(types, allocated, declaration, id)?,
            Some(ReservedEntity::Variant(id)) => define_variant(types, allocated, declaration, id)?,
            Some(ReservedEntity::OpaqueType(id)) => define_opaque(types, declaration, id)?,
            None if surface_kind(types, declaration)? == SurfaceDeclarationKind::Field => {}
            None => return Err(HeaderDefinitionError::InvalidSurface(declaration)),
        }
    }
    Ok(())
}

fn define_alias(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: nocter_model::TypeAliasId,
) -> Result<(), HeaderDefinitionError> {
    let definition = TypeAliasDeclaration::new(
        site(types, declaration)?,
        name(types, declaration)?,
        own_generics(types, declaration),
        types
            .alias_targets
            .get(&id)
            .copied()
            .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?,
        allocated.requirements[declaration.index()].clone(),
        target::gate(types, declaration)?,
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_type_alias(id, definition)?;
    Ok(())
}

fn define_instance(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: nocter_model::InstanceId,
) -> Result<(), HeaderDefinitionError> {
    let definition = InstanceDeclaration::new(
        site(types, declaration)?,
        pattern_type(types, declaration, 0)?,
        own_generics(types, declaration),
        allocated.requirements[declaration.index()].clone(),
        child_callables(types, declaration),
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_instance(id, definition)?;
    Ok(())
}

fn define_drop(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: nocter_model::DropId,
) -> Result<(), HeaderDefinitionError> {
    let definition = DropDeclaration::new(
        site(types, declaration)?,
        pattern_type(types, declaration, 0)?,
        own_generics(types, declaration),
        allocated.receivers[declaration.index()]
            .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?,
        allocated.bodies[declaration.index()]
            .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?,
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_drop(id, definition)?;
    Ok(())
}

fn define_test(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: nocter_model::TestId,
) -> Result<(), HeaderDefinitionError> {
    let definition = TestDeclaration::new(
        site(types, declaration)?,
        name(types, declaration)?,
        allocated.bodies[declaration.index()]
            .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?,
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_test(id, definition)?;
    Ok(())
}

fn define_variant(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: nocter_model::VariantId,
) -> Result<(), HeaderDefinitionError> {
    let owner = surface_owner(types, declaration)?;
    let Some(ReservedEntity::NominalType(owner)) = entity(types, owner) else {
        return Err(HeaderDefinitionError::InvalidOwner(declaration));
    };
    let definition = VariantDeclaration::new(
        site(types, declaration)?,
        owner,
        name(types, declaration)?,
        allocated.parameters[declaration.index()].clone(),
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_variant(id, definition)?;
    Ok(())
}

fn define_opaque(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    id: nocter_model::OpaqueTypeId,
) -> Result<(), HeaderDefinitionError> {
    let owner = surface_owner(types, declaration)?;
    let Some(ReservedEntity::Callable(owner)) = entity(types, owner) else {
        return Err(HeaderDefinitionError::InvalidOwner(declaration));
    };
    let opaque = types
        .opaque_results
        .get(&id)
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
    let definition = OpaqueTypeDeclaration::new(
        owner,
        opaque.generic_parameters(),
        opaque.interface().clone(),
        opaque.associated_types(),
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_opaque_type(id, definition)?;
    Ok(())
}

fn define_nominal(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: nocter_model::NominalTypeId,
) -> Result<(), HeaderDefinitionError> {
    let kind = surface_kind(types, declaration)?;
    let representation = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .contracts
        .representation(declaration)
        .unwrap_or(declaration);
    let shape = match kind {
        SurfaceDeclarationKind::Struct => {
            let tree = projection::tree(types, declaration)?;
            let node = surface_node(types, declaration)?;
            let copy_declared = syntax::direct_tokens(tree, node)
                .into_iter()
                .filter(|token| token.kind() == TokenKind::Identifier)
                .any(|token| {
                    types
                        .namespaces
                        .imports
                        .generics
                        .headers
                        .reserved
                        .source_map
                        .get(token.source())
                        .and_then(|source| source.text_at(token.range()))
                        == Some("copy")
                });
            NominalShape::Struct {
                copy_declared,
                fields: child_fields(types, allocated, representation),
            }
        }
        SurfaceDeclarationKind::Enum => NominalShape::Enum {
            variants: child_variants(types, representation),
        },
        _ => return Err(HeaderDefinitionError::InvalidSurface(declaration)),
    };
    let definition = NominalTypeDeclaration::new(
        site(types, declaration)?,
        name(types, declaration)?,
        own_generics(types, declaration),
        allocated.requirements[declaration.index()].clone(),
        shape,
        target::gate(types, declaration)?,
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_nominal_type(id, definition)?;
    Ok(())
}

fn define_interface(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: InterfaceId,
) -> Result<(), HeaderDefinitionError> {
    let definition = InterfaceDeclaration::new(
        site(types, declaration)?,
        name(types, declaration)?,
        own_generics(types, declaration),
        allocated.requirements[declaration.index()].clone(),
        child_associated_types(types, declaration),
        child_callables(types, declaration),
        target::gate(types, declaration)?,
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_interface(id, definition)?;
    Ok(())
}

fn define_associated(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: AssociatedTypeId,
) -> Result<(), HeaderDefinitionError> {
    let owner = surface_owner(types, declaration)?;
    let Some(ReservedEntity::Interface(interface)) = entity(types, owner) else {
        return Err(HeaderDefinitionError::InvalidOwner(declaration));
    };
    let definition = AssociatedTypeDeclaration::new(
        site(types, declaration)?,
        interface,
        name(types, declaration)?,
        allocated.requirements[declaration.index()].clone(),
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_associated_type(id, definition)?;
    Ok(())
}

fn define_construction(
    types: &mut PreparedTypes<'_>,
    _allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: nocter_model::ConstructionId,
) -> Result<(), HeaderDefinitionError> {
    let members = child_callables(types, declaration);
    let definition = ConstructionDeclaration::new(
        site(types, declaration)?,
        pattern_type(types, declaration, 0)?,
        own_generics(types, declaration),
        members,
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_construction(id, definition)?;
    Ok(())
}

fn define_conformance(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: nocter_model::ConformanceId,
) -> Result<(), HeaderDefinitionError> {
    let interface = match types
        .patterns
        .get(declaration.index())
        .and_then(|patterns| patterns.first())
    {
        Some(NormalizedDeclarationPattern::Interface(interface)) => interface.clone(),
        _ => return Err(HeaderDefinitionError::InvalidTypePattern(declaration)),
    };
    let bindings = conformance_bindings(types, declaration, &interface)?;
    let definition = ConformanceDeclaration::new(
        site(types, declaration)?,
        interface,
        pattern_type(types, declaration, 1)?,
        own_generics(types, declaration),
        allocated.requirements[declaration.index()].clone(),
        bindings,
        child_callables(types, declaration),
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_conformance(id, definition)?;
    Ok(())
}

fn conformance_bindings(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    interface: &InterfaceApplication,
) -> Result<Box<[AssociatedTypeBinding]>, HeaderDefinitionError> {
    let mut seen = HashMap::new();
    let mut result = Vec::new();
    for child in syntax::direct_nodes(
        projection::tree(types, declaration)?,
        surface_node(types, declaration)?,
        NodeKind::AssociatedTypeBinding,
    ) {
        let tree = projection::tree(types, declaration)?;
        let token = syntax::direct_identifier(tree, child)
            .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
        let name = projection::symbol(types, declaration, token)?;
        let associated =
            associated_by_name(types, interface.interface(), name).ok_or_else(|| {
                HeaderDefinitionError::from(DefinitionViolation::new(
                    DefinitionRule::UnknownAssociatedTypeBinding,
                    SyntaxOrigin::Token(token),
                ))
            })?;
        if let Some(first) = seen.insert(associated, token) {
            return Err(DefinitionViolation::duplicate(
                DefinitionRule::DuplicateAssociatedTypeBinding,
                SyntaxOrigin::Token(first),
                SyntaxOrigin::Token(token),
            )
            .into());
        }
        projection::token(
            types,
            declaration,
            SemanticEntity::AssociatedType(associated),
            SourceRole::Reference,
            token,
        )?;
        let origin = SourceOrigin::from_token(projection::tree(types, declaration)?, token)
            .map_err(|_| HeaderDefinitionError::InconsistentSource(token.source()))?;
        projection::occurrence_documentation(
            types,
            declaration,
            child,
            SemanticEntity::AssociatedType(associated),
            origin,
        )?;
        let ty_node =
            syntax::direct_node(projection::tree(types, declaration)?, child, NodeKind::Type)
                .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
        let ty = types
            .roots
            .get(&ty_node)
            .copied()
            .ok_or(HeaderDefinitionError::MissingType(ty_node))?;
        result.push(AssociatedTypeBinding::new(associated, ty));
    }
    Ok(result.into_boxed_slice())
}

fn associated_by_name(
    types: &PreparedTypes<'_>,
    interface: InterfaceId,
    sought: nocter_model::Symbol,
) -> Option<AssociatedTypeId> {
    (0..surface_count(types)).find_map(|index| {
        let declaration = SurfaceDeclarationId::from_index(index);
        let Some(ReservedEntity::AssociatedType(associated)) = entity(types, declaration) else {
            return None;
        };
        let owner = surface_owner(types, declaration).ok()?;
        (entity(types, owner) == Some(ReservedEntity::Interface(interface))
            && name(types, declaration).ok() == Some(sought))
        .then_some(associated)
    })
}

fn child_fields(
    types: &PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    owner: SurfaceDeclarationId,
) -> Box<[nocter_model::FieldId]> {
    child_surfaces(types, owner)
        .into_iter()
        .filter_map(|child| allocated.fields[child.index()])
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn child_variants(
    types: &PreparedTypes<'_>,
    owner: SurfaceDeclarationId,
) -> Box<[nocter_model::VariantId]> {
    child_entities(types, owner)
        .into_iter()
        .filter_map(|entity| match entity {
            ReservedEntity::Variant(id) => Some(id),
            _ => None,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn child_associated_types(
    types: &PreparedTypes<'_>,
    owner: SurfaceDeclarationId,
) -> Box<[AssociatedTypeId]> {
    child_entities(types, owner)
        .into_iter()
        .filter_map(|entity| match entity {
            ReservedEntity::AssociatedType(id) => Some(id),
            _ => None,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn child_callables(types: &PreparedTypes<'_>, owner: SurfaceDeclarationId) -> Box<[CallableId]> {
    let mut seen = HashSet::new();
    child_entities(types, owner)
        .into_iter()
        .filter_map(|entity| match entity {
            ReservedEntity::Callable(id) if seen.insert(id) => Some(id),
            _ => None,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn child_entities(types: &PreparedTypes<'_>, owner: SurfaceDeclarationId) -> Vec<ReservedEntity> {
    child_surfaces(types, owner)
        .into_iter()
        .filter_map(|child| entity(types, child))
        .collect()
}

fn child_surfaces(
    types: &PreparedTypes<'_>,
    owner: SurfaceDeclarationId,
) -> Vec<SurfaceDeclarationId> {
    let owner = representative(types, owner);
    (0..surface_count(types))
        .map(SurfaceDeclarationId::from_index)
        .filter(|child| {
            if representative(types, *child) != *child {
                return false;
            }
            types
                .namespaces
                .imports
                .generics
                .headers
                .reserved
                .declarations[child.index()]
            .owner()
            .map(|candidate| representative(types, candidate))
                == Some(owner)
        })
        .collect()
}

pub(super) fn own_generics(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Box<[nocter_model::GenericParameterId]> {
    types.namespaces.imports.generics.own[declaration.index()].clone()
}
