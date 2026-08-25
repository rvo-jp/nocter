use std::collections::HashMap;

use nocter_declarations::InterfaceApplication;
use nocter_model::{GenericParameterId, TypeAliasId, TypeId, TypeKind, TypeStore};

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclaration, SurfaceDeclarationId};

use super::{
    AliasDefinition, NormalizationContext, NormalizedDeclarationPattern, TypeNormalizationError,
};
use crate::types::{BoundDeclarationPattern, BoundRequirementKind, BoundTypeId};

pub(super) fn prepare_context(
    namespaces: &mut PreparedNamespaces<'_>,
    bound_alias_targets: &HashMap<TypeAliasId, BoundTypeId>,
    bound_patterns: &[Box<[BoundDeclarationPattern]>],
    bound_requirements: Box<[Box<[BoundRequirementKind]>]>,
) -> Result<NormalizationContext, TypeNormalizationError> {
    let reserved = &namespaces.imports.generics.headers.reserved;
    let declarations = reserved.declarations.clone();
    let entities = reserved.entities().to_vec().into_boxed_slice();
    let entity_declarations = reserved.entity_index.representatives().clone();
    let own_generics = namespaces.imports.generics.own.clone();
    let aliases = collect_aliases(&entities, &own_generics, bound_alias_targets)?;
    let associated = namespaces
        .imports
        .generics
        .headers
        .associated_types()
        .clone();
    let associated_surfaces = namespaces
        .imports
        .generics
        .headers
        .associated_type_declarations()
        .clone();
    let store = namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .types_mut();
    let generic_types = intern_generic_types(store, &own_generics)?;
    let patterns = normalize_patterns(store, bound_patterns, &generic_types)?;
    let self_types = normalize_self_types(
        store,
        &declarations,
        &entities,
        &own_generics,
        &generic_types,
        &patterns,
    )?;
    Ok(NormalizationContext {
        declarations,
        entities,
        entity_declarations,
        aliases,
        associated,
        associated_surfaces,
        self_types,
        patterns,
        bound_requirements,
    })
}

fn collect_aliases(
    entities: &[Option<ReservedEntity>],
    own_generics: &[Box<[GenericParameterId]>],
    targets: &HashMap<TypeAliasId, BoundTypeId>,
) -> Result<HashMap<TypeAliasId, AliasDefinition>, TypeNormalizationError> {
    let mut aliases = HashMap::new();
    for (index, entity) in entities.iter().copied().enumerate() {
        let Some(ReservedEntity::TypeAlias(alias)) = entity else {
            continue;
        };
        aliases.insert(
            alias,
            AliasDefinition {
                declaration: SurfaceDeclarationId::from_index(index),
                parameters: own_generics[index].clone(),
                target: targets
                    .get(&alias)
                    .copied()
                    .ok_or(TypeNormalizationError::MissingAlias(alias))?,
            },
        );
    }
    Ok(aliases)
}

fn intern_generic_types(
    store: &mut TypeStore,
    own_generics: &[Box<[GenericParameterId]>],
) -> Result<HashMap<GenericParameterId, TypeId>, TypeNormalizationError> {
    let mut types = HashMap::new();
    for parameters in own_generics {
        for parameter in parameters {
            if types.contains_key(parameter) {
                continue;
            }
            let ty = store
                .intern(TypeKind::GenericParameter(*parameter))
                .map_err(|_| TypeNormalizationError::InconsistentTypeStore)?;
            types.insert(*parameter, ty);
        }
    }
    Ok(types)
}

fn normalize_patterns(
    store: &mut TypeStore,
    patterns: &[Box<[BoundDeclarationPattern]>],
    generic_types: &HashMap<GenericParameterId, TypeId>,
) -> Result<Box<[Box<[NormalizedDeclarationPattern]>]>, TypeNormalizationError> {
    let normalized = patterns
        .iter()
        .map(|declaration| {
            declaration
                .iter()
                .map(|pattern| normalize_pattern(store, pattern, generic_types))
                .collect::<Result<Vec<_>, _>>()
                .map(Vec::into_boxed_slice)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(normalized.into_boxed_slice())
}

fn normalize_pattern(
    store: &mut TypeStore,
    pattern: &BoundDeclarationPattern,
    generic_types: &HashMap<GenericParameterId, TypeId>,
) -> Result<NormalizedDeclarationPattern, TypeNormalizationError> {
    Ok(match pattern {
        BoundDeclarationPattern::Builtin(builtin) => {
            NormalizedDeclarationPattern::Type(store.builtin(*builtin))
        }
        BoundDeclarationPattern::Slice(parameter) => {
            let element = generic_types[parameter];
            NormalizedDeclarationPattern::Type(
                store
                    .intern(TypeKind::Slice(element))
                    .map_err(|_| invalid_store())?,
            )
        }
        BoundDeclarationPattern::Nominal {
            definition,
            arguments,
        } => {
            let arguments: Box<_> = arguments
                .iter()
                .map(|parameter| generic_types[parameter])
                .collect();
            NormalizedDeclarationPattern::Type(
                store
                    .intern(TypeKind::Nominal {
                        definition: *definition,
                        arguments,
                    })
                    .map_err(|_| invalid_store())?,
            )
        }
        BoundDeclarationPattern::Interface {
            definition,
            arguments,
        } => NormalizedDeclarationPattern::Interface(InterfaceApplication::new(
            *definition,
            arguments
                .iter()
                .map(|parameter| generic_types[parameter])
                .collect::<Vec<_>>(),
        )),
    })
}

fn normalize_self_types(
    store: &mut TypeStore,
    declarations: &[SurfaceDeclaration],
    entities: &[Option<ReservedEntity>],
    own_generics: &[Box<[GenericParameterId]>],
    generic_types: &HashMap<GenericParameterId, TypeId>,
    patterns: &[Box<[NormalizedDeclarationPattern]>],
) -> Result<HashMap<ReservedEntity, TypeId>, TypeNormalizationError> {
    let mut result = HashMap::new();
    for (index, entity) in entities.iter().copied().enumerate() {
        let Some(entity) = entity else { continue };
        let ty = match entity {
            ReservedEntity::NominalType(definition) => Some(
                store
                    .intern(TypeKind::Nominal {
                        definition,
                        arguments: own_generics[index]
                            .iter()
                            .map(|parameter| generic_types[parameter])
                            .collect(),
                    })
                    .map_err(|_| invalid_store())?,
            ),
            ReservedEntity::Interface(interface) => Some(
                store
                    .intern(TypeKind::InterfaceSelf(interface))
                    .map_err(|_| invalid_store())?,
            ),
            ReservedEntity::Construction(_)
            | ReservedEntity::Instance(_)
            | ReservedEntity::Drop(_) => pattern_type(patterns.get(index).map(AsRef::as_ref), 0),
            ReservedEntity::Conformance(_) => {
                pattern_type(patterns.get(index).map(AsRef::as_ref), 1)
            }
            _ => None,
        };
        if let Some(ty) = ty {
            result.insert(entity, ty);
        } else if matches!(
            declarations[index].kind(),
            crate::SurfaceDeclarationKind::Construction
                | crate::SurfaceDeclarationKind::Instance
                | crate::SurfaceDeclarationKind::Conformance
                | crate::SurfaceDeclarationKind::Drop
        ) {
            return Err(TypeNormalizationError::InvalidSelf(entity));
        }
    }
    Ok(result)
}

const fn invalid_store() -> TypeNormalizationError {
    TypeNormalizationError::InconsistentTypeStore
}

fn pattern_type(patterns: Option<&[NormalizedDeclarationPattern]>, index: usize) -> Option<TypeId> {
    match patterns?.get(index)? {
        NormalizedDeclarationPattern::Type(ty) => Some(*ty),
        NormalizedDeclarationPattern::Interface(_) => None,
    }
}
