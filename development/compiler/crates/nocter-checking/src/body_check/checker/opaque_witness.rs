use nocter_declarations::{AssociatedTypeBinding, BodyOwner, InterfaceApplication};
use nocter_model::{BodyNodeId, BuiltinType, OpaqueTypeId, TypeId, TypeKind};
use nocter_syntax::NodeId;

use super::{BodyCheckError, BodyCheckInternalError, BodyChecker, BodyRule};
use crate::conformance::{proves_predicate, select_conformance};
use crate::type_relations::TypeSubstitution;
use crate::{
    BodySource, CheckedOpaqueWitness, CheckedOperation, CheckedPredicate, ExpectedEvidence,
    plan_expected_type,
};

pub(super) struct OpaqueResultState {
    definition: OpaqueTypeId,
    application: InterfaceApplication,
    associated_types: Box<[AssociatedTypeBinding]>,
    witness: Option<TypeId>,
}

struct OpaquePayload {
    definition: OpaqueTypeId,
    arguments: Box<[TypeId]>,
}

impl OpaqueResultState {
    pub(super) fn for_body(
        graph: &nocter_declarations::DeclarationGraph,
        types: &mut nocter_model::TypeStore,
        source: BodySource<'_>,
        result: TypeId,
    ) -> Result<Option<Self>, BodyCheckInternalError> {
        let Some(OpaquePayload {
            definition,
            arguments,
        }) = opaque_payload(types, result)?
        else {
            return Ok(None);
        };
        let BodyOwner::Callable(owner) = source.owner() else {
            return Err(BodyCheckInternalError::OpaqueWitnessPlanning);
        };
        let declaration = graph
            .declarations()
            .opaque_types()
            .get(definition)
            .cloned()
            .ok_or(BodyCheckInternalError::OpaqueWitnessPlanning)?;
        if declaration.owner() != owner || declaration.generic_parameters().len() != arguments.len()
        {
            return Err(BodyCheckInternalError::OpaqueWitnessPlanning);
        }
        let mut substitution = TypeSubstitution::default();
        for (parameter, argument) in declaration
            .generic_parameters()
            .iter()
            .copied()
            .zip(arguments)
        {
            substitution.bind_generic(parameter, argument);
        }
        let application = InterfaceApplication::new(
            declaration.interface().interface(),
            declaration
                .interface()
                .arguments()
                .iter()
                .map(|argument| substitution.apply_type(types, *argument))
                .collect::<Result<Vec<_>, _>>()
                .map_err(BodyCheckInternalError::BodyAssumptions)?,
        );
        let associated_types = declaration
            .associated_types()
            .iter()
            .map(|binding| {
                substitution
                    .apply_type(types, binding.ty())
                    .map(|ty| AssociatedTypeBinding::new(binding.declaration(), ty))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(BodyCheckInternalError::BodyAssumptions)?
            .into_boxed_slice();
        Ok(Some(Self {
            definition,
            application,
            associated_types,
            witness: None,
        }))
    }
}

impl BodyChecker<'_, '_> {
    pub(super) fn try_apply_opaque_witness(
        &mut self,
        node: NodeId,
        value: BodyNodeId,
        actual: TypeId,
        expected: TypeId,
    ) -> Result<Option<BodyNodeId>, BodyCheckError> {
        let Some(state) = self.opaque_result.as_ref() else {
            return Ok(None);
        };
        let definition = state.definition;
        let Some(witness) = successful_payload(self.types, actual)? else {
            return Ok(None);
        };
        if matches!(self.types.get(witness), Some(TypeKind::Opaque { .. })) {
            return Ok(None);
        }
        let Some(represented) = replace_opaque_payload(self.types, expected, definition, witness)?
        else {
            return Ok(None);
        };
        let Ok(plan) = plan_expected_type(self.types, represented, ExpectedEvidence::Typed(actual))
        else {
            return Ok(None);
        };
        self.validate_opaque_witness(node, witness)?;
        if self.flow_reachable {
            match self.opaque_result.as_ref().and_then(|state| state.witness) {
                Some(selected) if selected != witness => {
                    return Err(self.rule(BodyRule::InvalidOpaqueWitness, node)?);
                }
                Some(_) => {}
                None => {
                    self.opaque_result
                        .as_mut()
                        .ok_or(BodyCheckInternalError::OpaqueWitnessPlanning)?
                        .witness = Some(witness);
                }
            }
        }
        let represented = self.materialize_plan(node, plan, Some(value))?;
        self.add_node(
            node,
            expected,
            CheckedOperation::OpaqueWitness(CheckedOpaqueWitness::new(
                represented,
                definition,
                witness,
            )),
        )
        .map(Some)
    }

    pub(super) fn finish_opaque_witness(
        &self,
        node: NodeId,
    ) -> Result<Option<(OpaqueTypeId, TypeId)>, BodyCheckError> {
        let Some(state) = self.opaque_result.as_ref() else {
            return Ok(None);
        };
        let Some(witness) = state.witness else {
            return Err(self.rule(BodyRule::InvalidOpaqueWitness, node)?);
        };
        Ok(Some((state.definition, witness)))
    }

    fn validate_opaque_witness(
        &mut self,
        node: NodeId,
        witness: TypeId,
    ) -> Result<(), BodyCheckError> {
        let state = self
            .opaque_result
            .as_ref()
            .ok_or(BodyCheckInternalError::OpaqueWitnessPlanning)?;
        let application = state.application.clone();
        let associated_types = state.associated_types.clone();
        let predicate = CheckedPredicate::Capability {
            subject: witness,
            capability: nocter_declarations::StructuralCapability::Interface(application.clone()),
        };
        if !proves_predicate(
            self.types,
            self.conformances,
            &self.assumptions,
            &self.intrinsic_facts,
            &predicate,
        )
        .map_err(BodyCheckInternalError::BodyAssumptions)?
        {
            return Err(self.rule(BodyRule::InvalidOpaqueWitness, node)?);
        }
        let selected = select_conformance(
            self.types,
            self.conformances,
            &self.assumptions,
            &self.intrinsic_facts,
            witness,
            &application,
        )
        .map_err(BodyCheckInternalError::BodyAssumptions)?;
        if let Some(selected) = selected {
            let conformance = self
                .conformances
                .entries()
                .get(&selected.declaration())
                .cloned()
                .ok_or(BodyCheckInternalError::OpaqueWitnessPlanning)?;
            for expected in associated_types {
                let actual = conformance
                    .associated_type(expected.declaration())
                    .ok_or(BodyCheckInternalError::OpaqueWitnessPlanning)?;
                let actual = selected
                    .substitution()
                    .apply_type(self.types, actual)
                    .map_err(BodyCheckInternalError::BodyAssumptions)?;
                if actual != expected.ty() {
                    return Err(self.rule(BodyRule::InvalidOpaqueWitness, node)?);
                }
            }
            return Ok(());
        }
        for expected in associated_types {
            let projection = self
                .types
                .intern(TypeKind::AssociatedProjection {
                    base: witness,
                    associated: expected.declaration(),
                })
                .map_err(|_| BodyCheckInternalError::UnknownType(witness))?;
            if projection != expected.ty()
                && !self.assumptions.iter().any(|requirement| {
                    matches!(
                        requirement.predicate(),
                        CheckedPredicate::TypeEquality { left, right }
                            if (*left == projection && *right == expected.ty())
                                || (*right == projection && *left == expected.ty())
                    )
                })
            {
                return Err(self.rule(BodyRule::InvalidOpaqueWitness, node)?);
            }
        }
        Ok(())
    }
}

fn opaque_payload(
    types: &nocter_model::TypeStore,
    mut ty: TypeId,
) -> Result<Option<OpaquePayload>, BodyCheckInternalError> {
    loop {
        match types.get(ty) {
            Some(TypeKind::Optional(payload) | TypeKind::Fallible(payload)) => ty = *payload,
            Some(TypeKind::Opaque {
                definition,
                arguments,
            }) => {
                return Ok(Some(OpaquePayload {
                    definition: *definition,
                    arguments: arguments.clone(),
                }));
            }
            Some(_) => return Ok(None),
            None => return Err(BodyCheckInternalError::UnknownType(ty)),
        }
    }
}

fn successful_payload(
    types: &nocter_model::TypeStore,
    mut ty: TypeId,
) -> Result<Option<TypeId>, BodyCheckInternalError> {
    loop {
        match types.get(ty) {
            Some(TypeKind::Optional(payload) | TypeKind::Fallible(payload)) => ty = *payload,
            Some(TypeKind::Builtin(BuiltinType::Never | BuiltinType::Error)) => return Ok(None),
            Some(_) => return Ok(Some(ty)),
            None => return Err(BodyCheckInternalError::UnknownType(ty)),
        }
    }
}

fn replace_opaque_payload(
    types: &mut nocter_model::TypeStore,
    ty: TypeId,
    definition: OpaqueTypeId,
    witness: TypeId,
) -> Result<Option<TypeId>, BodyCheckInternalError> {
    match types.get(ty).cloned() {
        Some(TypeKind::Opaque {
            definition: actual, ..
        }) if actual == definition => Ok(Some(witness)),
        Some(TypeKind::Optional(payload)) => {
            replace_opaque_payload(types, payload, definition, witness)?
                .map(|payload| types.intern(TypeKind::Optional(payload)))
                .transpose()
                .map_err(|_| BodyCheckInternalError::UnknownType(payload))
        }
        Some(TypeKind::Fallible(payload)) => {
            replace_opaque_payload(types, payload, definition, witness)?
                .map(|payload| types.intern(TypeKind::Fallible(payload)))
                .transpose()
                .map_err(|_| BodyCheckInternalError::UnknownType(payload))
        }
        Some(_) => Ok(None),
        None => Err(BodyCheckInternalError::UnknownType(ty)),
    }
}
