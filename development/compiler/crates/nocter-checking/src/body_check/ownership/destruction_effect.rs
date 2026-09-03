use std::collections::{BTreeSet, HashSet};

use nocter_declarations::{DeclarationGraph, NominalShape, ParameterOwner};
use nocter_model::{ClosureId, DropId, OpaqueTypeId, ParameterId, TypeId, TypeKind, VariantId};

use crate::checked::CleanupEffect;
use crate::copyability::{CopyProofs, Copyability, CopyabilityTransaction};
use crate::type_relations::TypeSubstitution;
use crate::{BodyCheckInternalError, ClosureTable, DropTable, OpaqueWitnessTable};

/// Resolves the allocation-effect contract of exactly the destruction selected by ownership.
///
/// This resolver runs while the body semantic transaction is still open, so substituted field,
/// payload, and capture types become part of the same checked type generation. Consumers receive
/// only [`CleanupEffect`]; they cannot reopen declaration shapes to derive a different answer.
pub(super) struct DestructionEffectResolver<'program> {
    graph: &'program DeclarationGraph,
    types: &'program mut nocter_model::TypeTransaction,
    copyabilities: &'program mut CopyabilityTransaction,
    drops: &'program DropTable,
    closures: &'program ClosureTable,
    opaque_witnesses: &'program OpaqueWitnessTable,
    copy_proofs: &'program CopyProofs,
}

impl<'program> DestructionEffectResolver<'program> {
    pub(super) fn new(
        graph: &'program DeclarationGraph,
        types: &'program mut nocter_model::TypeTransaction,
        copyabilities: &'program mut CopyabilityTransaction,
        drops: &'program DropTable,
        closures: &'program ClosureTable,
        opaque_witnesses: &'program OpaqueWitnessTable,
        copy_proofs: &'program CopyProofs,
    ) -> Self {
        Self {
            graph,
            types,
            copyabilities,
            drops,
            closures,
            opaque_witnesses,
            copy_proofs,
        }
    }

    pub(super) fn resolve(&mut self, ty: TypeId) -> Result<CleanupEffect, BodyCheckInternalError> {
        let mut drops = BTreeSet::new();
        let mut unknown = false;
        self.visit_type(ty, &mut HashSet::new(), &mut drops, &mut unknown)?;
        Ok(CleanupEffect::new(
            drops.into_iter().collect::<Vec<_>>(),
            unknown,
        ))
    }

    pub(super) fn resolve_enum_residual(
        &mut self,
        ty: TypeId,
        variant: VariantId,
        payload: &[ParameterId],
    ) -> Result<CleanupEffect, BodyCheckInternalError> {
        let Some(TypeKind::Nominal {
            definition,
            arguments,
        }) = self.types.get(ty).cloned()
        else {
            return Err(BodyCheckInternalError::CleanupPlanning);
        };
        let nominal = self
            .graph
            .declarations()
            .nominal_types()
            .get(definition)
            .cloned()
            .ok_or(BodyCheckInternalError::CleanupPlanning)?;
        let NominalShape::Enum { variants } = nominal.shape() else {
            return Err(BodyCheckInternalError::CleanupPlanning);
        };
        if !variants.contains(&variant) || nominal.generic_parameters().len() != arguments.len() {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        let variant_id = variant;
        let variant = self
            .graph
            .declarations()
            .variants()
            .get(variant)
            .cloned()
            .ok_or(BodyCheckInternalError::CleanupPlanning)?;
        let selected = payload.iter().copied().collect::<BTreeSet<_>>();
        if selected.len() != payload.len()
            || selected
                .iter()
                .any(|parameter| !variant.payload().contains(parameter))
        {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        let substitution = bind(nominal.generic_parameters(), &arguments)?;
        let mut drops = BTreeSet::new();
        let mut unknown = false;
        let mut active = HashSet::new();
        for parameter in variant.payload().iter().copied() {
            if !selected.contains(&parameter) {
                continue;
            }
            let declaration = self
                .graph
                .declarations()
                .parameters()
                .get(parameter)
                .copied()
                .ok_or(BodyCheckInternalError::CleanupPlanning)?;
            if declaration.owner() != ParameterOwner::Variant(variant_id) {
                return Err(BodyCheckInternalError::CleanupPlanning);
            }
            let field_ty = substitution
                .apply_type(self.types, declaration.ty())
                .map_err(|_| BodyCheckInternalError::CleanupPlanning)?;
            self.visit_type(field_ty, &mut active, &mut drops, &mut unknown)?;
        }
        Ok(CleanupEffect::new(
            drops.into_iter().collect::<Vec<_>>(),
            unknown,
        ))
    }

    fn visit_type(
        &mut self,
        ty: TypeId,
        active: &mut HashSet<TypeId>,
        drops: &mut BTreeSet<DropId>,
        unknown: &mut bool,
    ) -> Result<(), BodyCheckInternalError> {
        if self
            .copyabilities
            .classify_with_proofs(self.graph, self.types, ty, self.copy_proofs)
            .map_err(BodyCheckInternalError::Copyability)?
            == Copyability::Copy
        {
            return Ok(());
        }
        if !active.insert(ty) {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        let kind = self
            .types
            .get(ty)
            .cloned()
            .ok_or(BodyCheckInternalError::CleanupPlanning)?;
        match kind {
            TypeKind::GenericParameter(_)
            | TypeKind::InterfaceSelf(_)
            | TypeKind::AssociatedProjection { .. } => *unknown = true,
            TypeKind::Opaque {
                definition,
                arguments,
            } => self.visit_opaque(definition, &arguments, active, drops, unknown)?,
            TypeKind::Nominal {
                definition,
                arguments,
            } => self.visit_nominal(definition, &arguments, active, drops, unknown)?,
            TypeKind::Closure {
                definition,
                arguments,
            } => self.visit_closure(definition, &arguments, active, drops, unknown)?,
            TypeKind::FixedArray { element, .. }
            | TypeKind::Optional(element)
            | TypeKind::Fallible(element) => {
                self.visit_type(element, active, drops, unknown)?;
            }
            TypeKind::PackEntry { key, value } => {
                self.visit_type(key, active, drops, unknown)?;
                self.visit_type(value, active, drops, unknown)?;
            }
            TypeKind::Tuple(elements) => {
                for element in elements.iter() {
                    self.visit_type(element, active, drops, unknown)?;
                }
            }
            TypeKind::Builtin(_)
            | TypeKind::Pointer(_)
            | TypeKind::Borrow { .. }
            | TypeKind::Slice(_)
            | TypeKind::Callable(_) => {}
        }
        active.remove(&ty);
        Ok(())
    }

    fn visit_nominal(
        &mut self,
        definition: nocter_model::NominalTypeId,
        arguments: &[TypeId],
        active: &mut HashSet<TypeId>,
        drops: &mut BTreeSet<DropId>,
        unknown: &mut bool,
    ) -> Result<(), BodyCheckInternalError> {
        if let Some(drop) = self.drops.get(definition) {
            drops.insert(drop);
        }
        let nominal = self
            .graph
            .declarations()
            .nominal_types()
            .get(definition)
            .cloned()
            .ok_or(BodyCheckInternalError::CleanupPlanning)?;
        let substitution = bind(nominal.generic_parameters(), arguments)?;
        let children = match nominal.shape() {
            NominalShape::Struct { fields, .. } => fields
                .iter()
                .map(|field| {
                    self.graph
                        .declarations()
                        .fields()
                        .get(*field)
                        .map(|field| field.ty())
                        .ok_or(BodyCheckInternalError::CleanupPlanning)
                })
                .collect::<Result<Vec<_>, _>>()?,
            NominalShape::Enum { variants } => variants
                .iter()
                .flat_map(|variant| {
                    self.graph
                        .declarations()
                        .variants()
                        .get(*variant)
                        .into_iter()
                        .flat_map(nocter_declarations::VariantDeclaration::payload)
                })
                .map(|parameter| {
                    self.graph
                        .declarations()
                        .parameters()
                        .get(*parameter)
                        .map(|parameter| parameter.ty())
                        .ok_or(BodyCheckInternalError::CleanupPlanning)
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        for child in children {
            let child = substitution
                .apply_type(self.types, child)
                .map_err(|_| BodyCheckInternalError::CleanupPlanning)?;
            self.visit_type(child, active, drops, unknown)?;
        }
        Ok(())
    }

    fn visit_closure(
        &mut self,
        definition: ClosureId,
        arguments: &[TypeId],
        active: &mut HashSet<TypeId>,
        drops: &mut BTreeSet<DropId>,
        unknown: &mut bool,
    ) -> Result<(), BodyCheckInternalError> {
        let closure = self
            .closures
            .get(definition)
            .cloned()
            .ok_or(BodyCheckInternalError::CleanupPlanning)?;
        let domain = self
            .graph
            .declarations()
            .body_generic_domain(closure.owner())
            .ok_or(BodyCheckInternalError::CleanupPlanning)?;
        let substitution = bind(&domain, arguments)?;
        for capture in closure.environment() {
            let capture = substitution
                .apply_type(self.types, capture.ty())
                .map_err(|_| BodyCheckInternalError::CleanupPlanning)?;
            self.visit_type(capture, active, drops, unknown)?;
        }
        Ok(())
    }

    fn visit_opaque(
        &mut self,
        definition: OpaqueTypeId,
        arguments: &[TypeId],
        active: &mut HashSet<TypeId>,
        drops: &mut BTreeSet<DropId>,
        unknown: &mut bool,
    ) -> Result<(), BodyCheckInternalError> {
        let opaque = self
            .graph
            .declarations()
            .opaque_types()
            .get(definition)
            .cloned()
            .ok_or(BodyCheckInternalError::CleanupPlanning)?;
        let substitution = bind(opaque.generic_parameters(), arguments)?;
        let witness = self
            .opaque_witnesses
            .get(definition)
            .ok_or(BodyCheckInternalError::CleanupPlanning)?;
        let witness = substitution
            .apply_type(self.types, witness)
            .map_err(|_| BodyCheckInternalError::CleanupPlanning)?;
        self.visit_type(witness, active, drops, unknown)
    }
}

fn bind(
    parameters: &[nocter_model::GenericParameterId],
    arguments: &[TypeId],
) -> Result<TypeSubstitution, BodyCheckInternalError> {
    if parameters.len() != arguments.len() {
        return Err(BodyCheckInternalError::CleanupPlanning);
    }
    let mut substitution = TypeSubstitution::default();
    for (parameter, argument) in parameters.iter().copied().zip(arguments.iter().copied()) {
        substitution.bind_generic(parameter, argument);
    }
    Ok(substitution)
}
