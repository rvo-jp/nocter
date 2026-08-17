use std::collections::{BTreeMap, BTreeSet};

use nocter_checking::{ConcreteDispatchResolver, TypeSubstitution, is_concrete_type};
use nocter_declarations::NominalShape;
use nocter_model::{FieldId, OpaqueTypeId, ParameterId, TypeId, TypeKind, VariantId};

use super::ExecutableProgramError;
use crate::TargetProgram;

/// One concrete field type in declaration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableFieldRepresentation {
    field: FieldId,
    ty: TypeId,
}

impl ExecutableFieldRepresentation {
    #[must_use]
    pub const fn field(self) -> FieldId {
        self.field
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// One concrete enum payload parameter type in declaration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutablePayloadRepresentation {
    parameter: ParameterId,
    ty: TypeId,
}

impl ExecutablePayloadRepresentation {
    #[must_use]
    pub const fn parameter(self) -> ParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// One concrete enum variant payload in declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableVariantRepresentation {
    variant: VariantId,
    payload: Box<[ExecutablePayloadRepresentation]>,
}

impl ExecutableVariantRepresentation {
    #[must_use]
    pub const fn variant(&self) -> VariantId {
        self.variant
    }

    #[must_use]
    pub const fn payload(&self) -> &[ExecutablePayloadRepresentation] {
        &self.payload
    }
}

/// Representation children already specialized before ABI layout begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutableTypeRepresentation {
    Struct {
        fields: Box<[ExecutableFieldRepresentation]>,
    },
    Enum {
        variants: Box<[ExecutableVariantRepresentation]>,
    },
    Opaque {
        definition: OpaqueTypeId,
        witness: TypeId,
    },
}

/// The sole concrete member/witness authority consumed by machine layout.
#[derive(Debug, Default)]
pub struct ExecutableTypeRepresentationTable {
    entries: BTreeMap<TypeId, ExecutableTypeRepresentation>,
}

impl ExecutableTypeRepresentationTable {
    #[must_use]
    pub fn get(&self, ty: TypeId) -> Option<&ExecutableTypeRepresentation> {
        self.entries.get(&ty)
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (TypeId, &ExecutableTypeRepresentation)> {
        self.entries
            .iter()
            .map(|(ty, representation)| (*ty, representation))
    }
}

pub(super) fn close_type_representations(
    target: &TargetProgram,
    resolver: &mut ConcreteDispatchResolver<'_>,
) -> Result<ExecutableTypeRepresentationTable, ExecutableProgramError> {
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
    Ok(ExecutableTypeRepresentationTable { entries })
}

fn close_nominal(
    target: &TargetProgram,
    resolver: &mut ConcreteDispatchResolver<'_>,
    ty: TypeId,
    definition: nocter_model::NominalTypeId,
    arguments: &[TypeId],
    pending: &mut BTreeSet<TypeId>,
) -> Result<ExecutableTypeRepresentation, ExecutableProgramError> {
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
                    Ok(ExecutableFieldRepresentation {
                        field,
                        ty: concrete,
                    })
                })
                .collect::<Result<Vec<_>, ExecutableProgramError>>()?
                .into_boxed_slice();
            Ok(ExecutableTypeRepresentation::Struct { fields })
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
                            Ok(ExecutablePayloadRepresentation {
                                parameter,
                                ty: concrete,
                            })
                        })
                        .collect::<Result<Vec<_>, ExecutableProgramError>>()?
                        .into_boxed_slice();
                    Ok(ExecutableVariantRepresentation { variant, payload })
                })
                .collect::<Result<Vec<_>, ExecutableProgramError>>()?
                .into_boxed_slice();
            Ok(ExecutableTypeRepresentation::Enum { variants })
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
) -> Result<ExecutableTypeRepresentation, ExecutableProgramError> {
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
    Ok(ExecutableTypeRepresentation::Opaque {
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
