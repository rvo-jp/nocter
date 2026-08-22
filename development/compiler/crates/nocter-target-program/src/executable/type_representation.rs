use std::collections::{BTreeMap, BTreeSet};

use nocter_checking::{ConcreteDispatchResolver, TypeSubstitution, is_concrete_type};
use nocter_declarations::NominalShape;
use nocter_model::{OpaqueTypeId, TypeId, TypeKind};
use nocter_runtime_contract::{
    RuntimeFieldRepresentation, RuntimePayloadRepresentation, RuntimeTypeRepresentation,
    RuntimeTypeRepresentationTable, RuntimeVariantRepresentation,
};

use super::ExecutableProgramError;
use crate::TargetProgram;

pub(super) fn close_type_representations(
    target: &TargetProgram,
    resolver: &mut ConcreteDispatchResolver<'_>,
) -> Result<RuntimeTypeRepresentationTable, ExecutableProgramError> {
    let candidates = resolver
        .types()
        .iter()
        .map(|(ty, _)| ty)
        .collect::<Vec<_>>();
    let mut pending = BTreeSet::new();
    for ty in candidates {
        if is_concrete_type(resolver.types(), ty)
            .map_err(|_| ExecutableProgramError::InvalidTypeRepresentation(ty))?
        {
            pending.insert(ty);
        }
    }

    let mut entries = BTreeMap::new();
    let mut completed = BTreeSet::new();
    while let Some(ty) = pending.pop_first() {
        if !completed.insert(ty) {
            continue;
        }
        let kind = resolver
            .types()
            .get(ty)
            .cloned()
            .ok_or(ExecutableProgramError::InvalidTypeRepresentation(ty))?;
        enqueue_structural_children(&kind, &mut pending);
        let representation = match kind {
            TypeKind::Nominal {
                definition,
                arguments,
            } => Some(close_nominal(
                target,
                resolver,
                ty,
                definition,
                &arguments,
                &mut pending,
            )?),
            TypeKind::Opaque {
                definition,
                arguments,
            } => Some(close_opaque(
                target,
                resolver,
                ty,
                definition,
                &arguments,
                &mut pending,
            )?),
            TypeKind::Builtin(_)
            | TypeKind::Pointer(_)
            | TypeKind::Borrow { .. }
            | TypeKind::Slice(_)
            | TypeKind::FixedArray { .. }
            | TypeKind::Closure { .. }
            | TypeKind::Callable(_)
            | TypeKind::Optional(_)
            | TypeKind::Fallible(_) => None,
            TypeKind::GenericParameter(_)
            | TypeKind::InterfaceSelf(_)
            | TypeKind::AssociatedProjection { .. } => {
                return Err(ExecutableProgramError::InvalidTypeRepresentation(ty));
            }
        };
        if let Some(representation) = representation {
            entries.insert(ty, representation);
        }
    }
    Ok(RuntimeTypeRepresentationTable::new(entries))
}

fn close_nominal(
    target: &TargetProgram,
    resolver: &mut ConcreteDispatchResolver<'_>,
    ty: TypeId,
    definition: nocter_model::NominalTypeId,
    arguments: &[TypeId],
    pending: &mut BTreeSet<TypeId>,
) -> Result<RuntimeTypeRepresentation, ExecutableProgramError> {
    let declarations = target.checked().graph().declarations();
    let nominal = declarations
        .nominal_types()
        .get(definition)
        .ok_or(ExecutableProgramError::InvalidTypeRepresentation(ty))?;
    let substitution = owner_substitution(ty, nominal.generic_parameters(), arguments)?;
    match nominal.shape() {
        NominalShape::Struct { fields, .. } => {
            let fields = fields
                .iter()
                .copied()
                .map(|field| {
                    let declaration = declarations
                        .fields()
                        .get(field)
                        .copied()
                        .ok_or(ExecutableProgramError::MissingRepresentationField(field))?;
                    let concrete = resolver.specialize_type(declaration.ty(), &substitution)?;
                    pending.insert(concrete);
                    Ok(RuntimeFieldRepresentation::new(field, concrete))
                })
                .collect::<Result<Vec<_>, ExecutableProgramError>>()?
                .into_boxed_slice();
            Ok(RuntimeTypeRepresentation::Struct { fields })
        }
        NominalShape::Enum { variants } => {
            let variants = variants
                .iter()
                .copied()
                .map(|variant| {
                    let declaration = declarations.variants().get(variant).ok_or(
                        ExecutableProgramError::MissingRepresentationVariant(variant),
                    )?;
                    let payload = declaration
                        .payload()
                        .iter()
                        .copied()
                        .map(|parameter| {
                            let declaration = declarations.parameters().get(parameter).ok_or(
                                ExecutableProgramError::MissingRepresentationParameter(parameter),
                            )?;
                            let concrete =
                                resolver.specialize_type(declaration.ty(), &substitution)?;
                            pending.insert(concrete);
                            Ok(RuntimePayloadRepresentation::new(parameter, concrete))
                        })
                        .collect::<Result<Vec<_>, ExecutableProgramError>>()?
                        .into_boxed_slice();
                    Ok(RuntimeVariantRepresentation::new(variant, payload))
                })
                .collect::<Result<Vec<_>, ExecutableProgramError>>()?
                .into_boxed_slice();
            Ok(RuntimeTypeRepresentation::Enum { variants })
        }
    }
}

fn close_opaque(
    target: &TargetProgram,
    resolver: &mut ConcreteDispatchResolver<'_>,
    ty: TypeId,
    definition: OpaqueTypeId,
    arguments: &[TypeId],
    pending: &mut BTreeSet<TypeId>,
) -> Result<RuntimeTypeRepresentation, ExecutableProgramError> {
    let declaration = target
        .checked()
        .graph()
        .declarations()
        .opaque_types()
        .get(definition)
        .ok_or(ExecutableProgramError::InvalidTypeRepresentation(ty))?;
    let substitution = owner_substitution(ty, declaration.generic_parameters(), arguments)?;
    let witness = target.checked().opaque_witnesses().get(definition).ok_or(
        ExecutableProgramError::MissingRepresentationWitness(definition),
    )?;
    let witness = resolver.specialize_type(witness, &substitution)?;
    pending.insert(witness);
    Ok(RuntimeTypeRepresentation::Opaque {
        definition,
        witness,
    })
}

fn owner_substitution(
    ty: TypeId,
    parameters: &[nocter_model::GenericParameterId],
    arguments: &[TypeId],
) -> Result<TypeSubstitution, ExecutableProgramError> {
    if parameters.len() != arguments.len() {
        return Err(ExecutableProgramError::InvalidTypeRepresentation(ty));
    }
    let mut substitution = TypeSubstitution::default();
    for (parameter, argument) in parameters.iter().copied().zip(arguments.iter().copied()) {
        substitution.bind_generic(parameter, argument);
    }
    Ok(substitution)
}

fn enqueue_structural_children(kind: &TypeKind, pending: &mut BTreeSet<TypeId>) {
    match kind {
        TypeKind::Nominal { arguments, .. }
        | TypeKind::Opaque { arguments, .. }
        | TypeKind::Closure { arguments, .. } => pending.extend(arguments.iter().copied()),
        TypeKind::AssociatedProjection { base, .. }
        | TypeKind::Pointer(base)
        | TypeKind::Borrow { referent: base, .. }
        | TypeKind::Slice(base)
        | TypeKind::FixedArray { element: base, .. }
        | TypeKind::Optional(base)
        | TypeKind::Fallible(base) => {
            pending.insert(*base);
        }
        TypeKind::Callable(contract) => {
            pending.extend(contract.parameters().iter().copied());
            pending.insert(contract.result());
        }
        TypeKind::Builtin(_) | TypeKind::GenericParameter(_) | TypeKind::InterfaceSelf(_) => {}
    }
}
