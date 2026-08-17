use std::collections::BTreeSet;
use std::fmt;

use nocter_declarations::{NominalShape, ParameterOwner};
use nocter_model::{BodyId, CaptureId, FieldId, OpaqueTypeId, TypeId, TypeKind, VariantId};

use crate::concrete_dispatch::ConcreteDispatchResolver;
use crate::instance_operations::{InstanceSelectionError, selected_generic_arguments};
use crate::type_relations::{SubstitutionError, TypeSubstitution, match_type_pattern};
use crate::{Copyability, CopyabilityError, DropSelection, is_concrete_type};

/// One complete runtime destruction shape after generic specialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConcreteDestructionPlan {
    ty: TypeId,
    kind: ConcreteDestructionKind,
}

impl ConcreteDestructionPlan {
    const fn new(ty: TypeId, kind: ConcreteDestructionKind) -> Self {
        Self { ty, kind }
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn kind(&self) -> &ConcreteDestructionKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConcreteDestructionKind {
    Struct {
        drop: Option<DropSelection>,
        fields: Box<[ConcreteFieldDestruction]>,
    },
    Enum {
        drop: Option<DropSelection>,
        variants: Box<[ConcreteVariantDestruction]>,
    },
    FixedArray {
        length: u64,
        element: Box<ConcreteDestructionPlan>,
    },
    Optional(Box<ConcreteDestructionPlan>),
    Fallible(Box<ConcreteDestructionPlan>),
    Closure(Box<[ConcreteCaptureDestruction]>),
    Opaque {
        definition: OpaqueTypeId,
        witness: TypeId,
        plan: Box<ConcreteDestructionPlan>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConcreteFieldDestruction {
    field: FieldId,
    plan: ConcreteDestructionPlan,
}

impl ConcreteFieldDestruction {
    #[must_use]
    pub const fn field(&self) -> FieldId {
        self.field
    }

    #[must_use]
    pub const fn plan(&self) -> &ConcreteDestructionPlan {
        &self.plan
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConcreteVariantDestruction {
    variant: VariantId,
    payload: Box<[ConcretePayloadDestruction]>,
}

impl ConcreteVariantDestruction {
    #[must_use]
    pub const fn variant(&self) -> VariantId {
        self.variant
    }

    #[must_use]
    pub const fn payload(&self) -> &[ConcretePayloadDestruction] {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConcretePayloadDestruction {
    parameter: nocter_model::ParameterId,
    plan: ConcreteDestructionPlan,
}

impl ConcretePayloadDestruction {
    #[must_use]
    pub const fn parameter(&self) -> nocter_model::ParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn plan(&self) -> &ConcreteDestructionPlan {
        &self.plan
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConcreteCaptureDestruction {
    capture: CaptureId,
    plan: ConcreteDestructionPlan,
}

impl ConcreteCaptureDestruction {
    #[must_use]
    pub const fn capture(&self) -> CaptureId {
        self.capture
    }

    #[must_use]
    pub const fn plan(&self) -> &ConcreteDestructionPlan {
        &self.plan
    }
}

impl ConcreteDispatchResolver<'_> {
    /// Interns one newly assembled concrete type in the specialization store.
    ///
    /// # Errors
    ///
    /// Rejects an unknown referenced type or a kind that still contains symbolic components.
    pub fn intern_concrete(&mut self, kind: TypeKind) -> Result<TypeId, ConcreteDestructionError> {
        let ty = self
            .types
            .intern(kind)
            .map_err(|unknown| ConcreteDestructionError::UnknownType(unknown.id()))?;
        if !is_concrete_type(&self.types, ty)? {
            return Err(ConcreteDestructionError::SymbolicType(ty));
        }
        Ok(ty)
    }

    /// Applies one enclosing specialization and requires a fully concrete result.
    ///
    /// # Errors
    ///
    /// Returns a typed invariant failure when substitution cannot rebuild the type or leaves a
    /// symbolic identity at the executable boundary.
    pub fn specialize_type(
        &mut self,
        ty: TypeId,
        enclosing: &TypeSubstitution,
    ) -> Result<TypeId, ConcreteDestructionError> {
        let ty = enclosing.apply_type(&mut self.types, ty)?;
        if !is_concrete_type(&self.types, ty)? {
            return Err(ConcreteDestructionError::SymbolicType(ty));
        }
        Ok(ty)
    }

    /// Resolves the exact recursive glue required to destroy one specialized type.
    ///
    /// # Errors
    ///
    /// Returns a typed invariant failure when specialization is incomplete, a checked semantic
    /// reference is missing, or recursive value storage has no finite destruction shape.
    pub fn resolve_destruction(
        &mut self,
        ty: TypeId,
        enclosing: &TypeSubstitution,
    ) -> Result<Option<ConcreteDestructionPlan>, ConcreteDestructionError> {
        let ty = self.specialize_type(ty, enclosing)?;
        self.plan_type(ty, &mut BTreeSet::new())
    }

    /// Resolves only the still-initialized payload fields of one consumed enum pattern.
    ///
    /// The enum's own drop body is deliberately absent: checked ownership runs it before the first
    /// named move-only payload leaves the value.
    ///
    /// # Errors
    ///
    /// Returns a typed invariant failure when the specialized type is not the selected enum,
    /// payload identities do not belong to the selected variant, or child destruction is invalid.
    pub fn resolve_enum_residual(
        &mut self,
        ty: TypeId,
        variant: VariantId,
        payload: &[nocter_model::ParameterId],
        enclosing: &TypeSubstitution,
    ) -> Result<Option<ConcreteDestructionPlan>, ConcreteDestructionError> {
        let ty = self.specialize_type(ty, enclosing)?;
        let Some(TypeKind::Nominal {
            definition,
            arguments,
        }) = self.types.get(ty).cloned()
        else {
            return Err(ConcreteDestructionError::InvalidEnumResidual(ty));
        };
        let nominal = self
            .program
            .graph()
            .declarations()
            .nominal_types()
            .get(definition)
            .cloned()
            .ok_or(ConcreteDestructionError::MissingNominal(definition))?;
        let NominalShape::Enum { variants } = nominal.shape() else {
            return Err(ConcreteDestructionError::InvalidEnumResidual(ty));
        };
        if !variants.contains(&variant) || nominal.generic_parameters().len() != arguments.len() {
            return Err(ConcreteDestructionError::InvalidEnumResidual(ty));
        }
        let mut substitution = TypeSubstitution::default();
        for (parameter, argument) in nominal
            .generic_parameters()
            .iter()
            .copied()
            .zip(arguments.iter().copied())
        {
            substitution.bind_generic(parameter, argument);
        }
        let variant_declaration = self
            .program
            .graph()
            .declarations()
            .variants()
            .get(variant)
            .cloned()
            .ok_or(ConcreteDestructionError::MissingVariant(variant))?;
        let selected = payload.iter().copied().collect::<BTreeSet<_>>();
        if selected.len() != payload.len()
            || selected
                .iter()
                .any(|parameter| !variant_declaration.payload().contains(parameter))
        {
            return Err(ConcreteDestructionError::InvalidEnumResidual(ty));
        }
        let mut plans = Vec::new();
        let mut active = BTreeSet::new();
        for parameter in variant_declaration.payload().iter().rev().copied() {
            if !selected.contains(&parameter) {
                continue;
            }
            let declaration = self
                .program
                .graph()
                .declarations()
                .parameters()
                .get(parameter)
                .copied()
                .ok_or(ConcreteDestructionError::MissingPayload(parameter))?;
            if declaration.owner() != ParameterOwner::Variant(variant) {
                return Err(ConcreteDestructionError::PayloadOwnerMismatch(parameter));
            }
            let field_ty = substitution.apply_type(&mut self.types, declaration.ty())?;
            if let Some(plan) = self.plan_type(field_ty, &mut active)? {
                plans.push(ConcretePayloadDestruction { parameter, plan });
            }
        }
        Ok((!plans.is_empty()).then(|| {
            ConcreteDestructionPlan::new(
                ty,
                ConcreteDestructionKind::Enum {
                    drop: None,
                    variants: vec![ConcreteVariantDestruction {
                        variant,
                        payload: plans.into_boxed_slice(),
                    }]
                    .into_boxed_slice(),
                },
            )
        }))
    }

    fn plan_type(
        &mut self,
        ty: TypeId,
        active: &mut BTreeSet<TypeId>,
    ) -> Result<Option<ConcreteDestructionPlan>, ConcreteDestructionError> {
        if let Some(plan) = self.destructions.get(&ty) {
            return Ok(plan.clone());
        }
        if !active.insert(ty) {
            return Err(ConcreteDestructionError::RecursiveType(ty));
        }
        if self
            .copyabilities
            .classify(self.program.graph(), &mut self.types, ty)?
            == Copyability::Copy
        {
            active.remove(&ty);
            self.destructions.insert(ty, None);
            return Ok(None);
        }
        let kind = self
            .types
            .get(ty)
            .cloned()
            .ok_or(ConcreteDestructionError::UnknownType(ty))?;
        let plan = match kind {
            TypeKind::Nominal {
                definition,
                arguments,
            } => self.plan_nominal(ty, definition, &arguments, active)?,
            TypeKind::FixedArray { element, length } => {
                self.plan_type(element, active)?.map(|element| {
                    ConcreteDestructionPlan::new(
                        ty,
                        ConcreteDestructionKind::FixedArray {
                            length,
                            element: Box::new(element),
                        },
                    )
                })
            }
            TypeKind::Optional(payload) => self.plan_type(payload, active)?.map(|payload| {
                ConcreteDestructionPlan::new(
                    ty,
                    ConcreteDestructionKind::Optional(Box::new(payload)),
                )
            }),
            TypeKind::Fallible(payload) => self.plan_type(payload, active)?.map(|payload| {
                ConcreteDestructionPlan::new(
                    ty,
                    ConcreteDestructionKind::Fallible(Box::new(payload)),
                )
            }),
            TypeKind::Closure {
                definition,
                arguments,
            } => self.plan_closure(ty, definition, &arguments, active)?,
            TypeKind::Opaque {
                definition,
                arguments,
            } => self.plan_opaque(ty, definition, &arguments, active)?,
            TypeKind::Builtin(_)
            | TypeKind::Pointer(_)
            | TypeKind::Borrow { .. }
            | TypeKind::Slice(_)
            | TypeKind::Callable(_) => None,
            TypeKind::GenericParameter(_)
            | TypeKind::InterfaceSelf(_)
            | TypeKind::AssociatedProjection { .. } => {
                return Err(ConcreteDestructionError::SymbolicType(ty));
            }
        };
        active.remove(&ty);
        self.destructions.insert(ty, plan.clone());
        Ok(plan)
    }

    fn plan_nominal(
        &mut self,
        ty: TypeId,
        definition: nocter_model::NominalTypeId,
        arguments: &[TypeId],
        active: &mut BTreeSet<TypeId>,
    ) -> Result<Option<ConcreteDestructionPlan>, ConcreteDestructionError> {
        let declaration = self
            .program
            .graph()
            .declarations()
            .nominal_types()
            .get(definition)
            .cloned()
            .ok_or(ConcreteDestructionError::MissingNominal(definition))?;
        if declaration.generic_parameters().len() != arguments.len() {
            return Err(ConcreteDestructionError::InvalidNominalDomain(definition));
        }
        let mut substitution = TypeSubstitution::default();
        for (parameter, argument) in declaration
            .generic_parameters()
            .iter()
            .copied()
            .zip(arguments.iter().copied())
        {
            substitution.bind_generic(parameter, argument);
        }
        let drop = self.select_drop(definition, ty)?;
        let kind = match declaration.shape() {
            NominalShape::Struct { fields, .. } => {
                let fields = self.plan_fields(definition, fields, &substitution, active)?;
                if drop.is_none() && fields.is_empty() {
                    return Ok(None);
                }
                ConcreteDestructionKind::Struct {
                    drop,
                    fields: fields.into_boxed_slice(),
                }
            }
            NominalShape::Enum { variants } => {
                let variants = self.plan_variants(definition, variants, &substitution, active)?;
                if drop.is_none() && variants.is_empty() {
                    return Ok(None);
                }
                ConcreteDestructionKind::Enum {
                    drop,
                    variants: variants.into_boxed_slice(),
                }
            }
        };
        Ok(Some(ConcreteDestructionPlan::new(ty, kind)))
    }

    fn plan_fields(
        &mut self,
        owner: nocter_model::NominalTypeId,
        fields: &[FieldId],
        substitution: &TypeSubstitution,
        active: &mut BTreeSet<TypeId>,
    ) -> Result<Vec<ConcreteFieldDestruction>, ConcreteDestructionError> {
        let mut plans = Vec::new();
        for field in fields.iter().rev().copied() {
            let declaration = self
                .program
                .graph()
                .declarations()
                .fields()
                .get(field)
                .copied()
                .ok_or(ConcreteDestructionError::MissingField(field))?;
            if declaration.owner() != owner {
                return Err(ConcreteDestructionError::FieldOwnerMismatch(field));
            }
            let ty = substitution.apply_type(&mut self.types, declaration.ty())?;
            if let Some(plan) = self.plan_type(ty, active)? {
                plans.push(ConcreteFieldDestruction { field, plan });
            }
        }
        Ok(plans)
    }

    fn plan_variants(
        &mut self,
        owner: nocter_model::NominalTypeId,
        variants: &[VariantId],
        substitution: &TypeSubstitution,
        active: &mut BTreeSet<TypeId>,
    ) -> Result<Vec<ConcreteVariantDestruction>, ConcreteDestructionError> {
        let mut plans = Vec::new();
        for variant in variants {
            let declaration = self
                .program
                .graph()
                .declarations()
                .variants()
                .get(*variant)
                .cloned()
                .ok_or(ConcreteDestructionError::MissingVariant(*variant))?;
            if declaration.owner() != owner {
                return Err(ConcreteDestructionError::VariantOwnerMismatch(*variant));
            }
            let mut payload = Vec::new();
            for parameter in declaration.payload().iter().rev().copied() {
                let declaration = self
                    .program
                    .graph()
                    .declarations()
                    .parameters()
                    .get(parameter)
                    .copied()
                    .ok_or(ConcreteDestructionError::MissingPayload(parameter))?;
                if declaration.owner() != ParameterOwner::Variant(*variant) {
                    return Err(ConcreteDestructionError::PayloadOwnerMismatch(parameter));
                }
                let ty = substitution.apply_type(&mut self.types, declaration.ty())?;
                if let Some(plan) = self.plan_type(ty, active)? {
                    payload.push(ConcretePayloadDestruction { parameter, plan });
                }
            }
            if !payload.is_empty() {
                plans.push(ConcreteVariantDestruction {
                    variant: *variant,
                    payload: payload.into_boxed_slice(),
                });
            }
        }
        Ok(plans)
    }

    fn plan_closure(
        &mut self,
        ty: TypeId,
        definition: nocter_model::ClosureId,
        arguments: &[TypeId],
        active: &mut BTreeSet<TypeId>,
    ) -> Result<Option<ConcreteDestructionPlan>, ConcreteDestructionError> {
        let closure = self
            .program
            .closures()
            .get(definition)
            .cloned()
            .ok_or(ConcreteDestructionError::MissingClosure(definition))?;
        let domain = self
            .program
            .graph()
            .declarations()
            .body_generic_domain(closure.owner())
            .ok_or(ConcreteDestructionError::MissingBody(closure.owner()))?;
        if domain.len() != arguments.len() {
            return Err(ConcreteDestructionError::InvalidClosureDomain(definition));
        }
        let mut substitution = TypeSubstitution::default();
        for (parameter, argument) in domain.iter().copied().zip(arguments.iter().copied()) {
            substitution.bind_generic(parameter, argument);
        }
        let mut captures = Vec::new();
        for capture in closure.environment().iter().rev().copied() {
            let capture_type = substitution.apply_type(&mut self.types, capture.ty())?;
            if let Some(plan) = self.plan_type(capture_type, active)? {
                captures.push(ConcreteCaptureDestruction {
                    capture: capture.binding(),
                    plan,
                });
            }
        }
        Ok((!captures.is_empty()).then(|| {
            ConcreteDestructionPlan::new(
                ty,
                ConcreteDestructionKind::Closure(captures.into_boxed_slice()),
            )
        }))
    }

    fn plan_opaque(
        &mut self,
        ty: TypeId,
        definition: OpaqueTypeId,
        arguments: &[TypeId],
        active: &mut BTreeSet<TypeId>,
    ) -> Result<Option<ConcreteDestructionPlan>, ConcreteDestructionError> {
        let opaque = self
            .program
            .graph()
            .declarations()
            .opaque_types()
            .get(definition)
            .cloned()
            .ok_or(ConcreteDestructionError::MissingOpaque(definition))?;
        if opaque.generic_parameters().len() != arguments.len() {
            return Err(ConcreteDestructionError::InvalidOpaqueDomain(definition));
        }
        let mut substitution = TypeSubstitution::default();
        for (parameter, argument) in opaque
            .generic_parameters()
            .iter()
            .copied()
            .zip(arguments.iter().copied())
        {
            substitution.bind_generic(parameter, argument);
        }
        let witness = self
            .program
            .opaque_witnesses()
            .get(definition)
            .ok_or(ConcreteDestructionError::MissingOpaqueWitness(definition))?;
        let witness = substitution.apply_type(&mut self.types, witness)?;
        Ok(self.plan_type(witness, active)?.map(|plan| {
            ConcreteDestructionPlan::new(
                ty,
                ConcreteDestructionKind::Opaque {
                    definition,
                    witness,
                    plan: Box::new(plan),
                },
            )
        }))
    }

    fn select_drop(
        &mut self,
        definition: nocter_model::NominalTypeId,
        ty: TypeId,
    ) -> Result<Option<DropSelection>, ConcreteDestructionError> {
        let Some(drop) = self.program.drops().get(definition) else {
            return Ok(None);
        };
        let declaration = self
            .program
            .graph()
            .declarations()
            .drops()
            .get(drop)
            .cloned()
            .ok_or(ConcreteDestructionError::MissingDrop(drop))?;
        let bindings = match_type_pattern(&self.types, declaration.target(), ty)?
            .ok_or(ConcreteDestructionError::InvalidDropTarget(drop))?;
        let mut substitution = TypeSubstitution::default();
        for (parameter, ty) in bindings.iter() {
            substitution.bind_generic(parameter, ty);
        }
        let arguments = selected_generic_arguments(
            &mut self.types,
            declaration.generic_parameters(),
            &substitution,
        )?;
        for argument in arguments.as_slice() {
            if !is_concrete_type(&self.types, argument.ty())? {
                return Err(ConcreteDestructionError::SymbolicType(argument.ty()));
            }
        }
        Ok(Some(DropSelection::new(drop, arguments)))
    }
}

#[derive(Debug)]
pub enum ConcreteDestructionError {
    UnknownType(TypeId),
    SymbolicType(TypeId),
    RecursiveType(TypeId),
    MissingNominal(nocter_model::NominalTypeId),
    InvalidNominalDomain(nocter_model::NominalTypeId),
    MissingField(FieldId),
    FieldOwnerMismatch(FieldId),
    MissingVariant(VariantId),
    VariantOwnerMismatch(VariantId),
    MissingPayload(nocter_model::ParameterId),
    PayloadOwnerMismatch(nocter_model::ParameterId),
    MissingDrop(nocter_model::DropId),
    InvalidDropTarget(nocter_model::DropId),
    MissingClosure(nocter_model::ClosureId),
    MissingBody(BodyId),
    InvalidClosureDomain(nocter_model::ClosureId),
    MissingOpaque(OpaqueTypeId),
    InvalidOpaqueDomain(OpaqueTypeId),
    MissingOpaqueWitness(OpaqueTypeId),
    InvalidEnumResidual(TypeId),
    Substitution(SubstitutionError),
    Selection(InstanceSelectionError),
    Copyability(CopyabilityError),
}

impl fmt::Display for ConcreteDestructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "concrete destruction invariant failed: {self:?}")
    }
}

impl std::error::Error for ConcreteDestructionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Substitution(error) => Some(error),
            Self::Selection(error) => Some(error),
            Self::Copyability(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SubstitutionError> for ConcreteDestructionError {
    fn from(error: SubstitutionError) -> Self {
        Self::Substitution(error)
    }
}

impl From<InstanceSelectionError> for ConcreteDestructionError {
    fn from(error: InstanceSelectionError) -> Self {
        Self::Selection(error)
    }
}

impl From<CopyabilityError> for ConcreteDestructionError {
    fn from(error: CopyabilityError) -> Self {
        Self::Copyability(error)
    }
}

#[cfg(test)]
mod tests;
