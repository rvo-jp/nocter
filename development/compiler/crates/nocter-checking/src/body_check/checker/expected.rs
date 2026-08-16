use nocter_model::{BodyNodeId, BorrowCapability, PlaceId, TypeId, TypeKind};
use nocter_syntax::NodeId;

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::instance_operations::InstanceOperationSelector;
use crate::{
    BorrowConversionImplementation, BorrowConversionPreparation, CheckedBorrowConversion,
    CheckedOperation, CheckedOutcome, ExpectedBase, ExpectedEvidence, ExpectedTypeError,
    ExpectedTypePlan, OutcomeLayer, plan_expected_type,
};

impl BodyChecker<'_, '_> {
    pub(super) fn apply_expected_place(
        &mut self,
        node: NodeId,
        place: PlaceId,
        actual: TypeId,
        expected: TypeId,
    ) -> Result<BodyNodeId, BodyCheckError> {
        match plan_expected_type(self.types, expected, ExpectedEvidence::Typed(actual)) {
            Ok(plan) => {
                let value = self.add_node(node, actual, CheckedOperation::Copy(place))?;
                self.materialize_plan(node, plan, Some(value))
            }
            Err(ExpectedTypeError::Mismatch { .. }) => {
                let value = self.add_node(node, actual, CheckedOperation::Place(place))?;
                self.apply_borrow_conversion(node, value, actual, expected)
            }
            Err(error) => Err(self.expected_error(node, error)),
        }
    }

    pub(super) fn apply_expected(
        &mut self,
        node: NodeId,
        value: BodyNodeId,
        expected: TypeId,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let actual = self.node_type(value)?;
        match plan_expected_type(self.types, expected, ExpectedEvidence::Typed(actual)) {
            Ok(plan) => self.materialize_plan(node, plan, Some(value)),
            Err(ExpectedTypeError::Mismatch { .. }) => {
                self.apply_borrow_conversion(node, value, actual, expected)
            }
            Err(error) => Err(self.expected_error(node, error)),
        }
    }

    fn apply_borrow_conversion(
        &mut self,
        node: NodeId,
        value: BodyNodeId,
        actual: TypeId,
        expected: TypeId,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let mut target = expected;
        let mut outer = Vec::new();
        loop {
            if let Some(conversion) = self.select_borrow_conversion(actual, target)? {
                let converted = self.add_node(
                    node,
                    target,
                    CheckedOperation::BorrowConversion(CheckedBorrowConversion::new(
                        value,
                        target,
                        conversion.0,
                        conversion.1,
                    )),
                )?;
                outer.reverse();
                return self.materialize_injections(node, converted, &outer);
            }
            match self.types.get(target) {
                Some(TypeKind::Optional(payload)) => {
                    outer.push(OutcomeLayer::Optional);
                    target = *payload;
                }
                Some(TypeKind::Fallible(payload)) => {
                    outer.push(OutcomeLayer::Fallible);
                    target = *payload;
                }
                Some(_) => return Err(self.rule(BodyRule::TypeMismatch, node)?),
                None => return Err(BodyCheckInternalError::UnknownType(target).into()),
            }
        }
    }

    fn select_borrow_conversion(
        &mut self,
        actual: TypeId,
        expected: TypeId,
    ) -> Result<Option<(BorrowConversionPreparation, BorrowConversionImplementation)>, BodyCheckError>
    {
        let Some((source_capability, source)) = borrowed_type(self.types, actual) else {
            return Ok(None);
        };
        let Some((target_capability, target)) = borrowed_type(self.types, expected) else {
            return Ok(None);
        };
        if source == target
            && source_capability == BorrowCapability::ReadWrite
            && target_capability == BorrowCapability::Readonly
        {
            return Ok(Some((
                BorrowConversionPreparation::WeakenReadwrite,
                BorrowConversionImplementation::CapabilityWeakening,
            )));
        }
        let candidates = {
            let mut selector = InstanceOperationSelector::new(
                self.graph,
                self.types,
                self.conformances,
                self.copyabilities,
                self.instance_operations,
                &self.assumptions,
                self.source.module(),
            );
            selector
                .select_borrow_coercions(source, source_capability, target_capability)
                .map_err(BodyCheckInternalError::from)?
                .into_iter()
                .filter(|candidate| candidate.target() == target)
                .collect::<Vec<_>>()
        };
        let mut candidates = candidates.into_iter();
        let Some(selected) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() {
            return Err(BodyCheckInternalError::ExpectedConversion.into());
        }
        let preparation = match (source_capability, selected.receiver_capability()) {
            (BorrowCapability::Readonly, BorrowCapability::Readonly) => {
                BorrowConversionPreparation::PreserveReadonly
            }
            (BorrowCapability::ReadWrite, BorrowCapability::ReadWrite) => {
                BorrowConversionPreparation::PreserveReadwrite
            }
            (BorrowCapability::ReadWrite, BorrowCapability::Readonly) => {
                BorrowConversionPreparation::WeakenReadwrite
            }
            (BorrowCapability::Readonly, BorrowCapability::ReadWrite) => {
                return Err(BodyCheckInternalError::ExpectedConversion.into());
            }
        };
        Ok(Some((
            preparation,
            BorrowConversionImplementation::Selected(selected.selection().clone()),
        )))
    }

    pub(super) fn materialize_plan(
        &mut self,
        node: NodeId,
        plan: ExpectedTypePlan,
        payload: Option<BodyNodeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let (base, injections) = plan.into_parts();
        let current = match base {
            ExpectedBase::Exact(_) | ExpectedBase::Diverges(_) => {
                payload.ok_or(BodyCheckInternalError::InvalidSyntax(node))?
            }
            ExpectedBase::Absent(ty) => {
                self.add_node(node, ty, CheckedOperation::Outcome(CheckedOutcome::Absent))?
            }
            ExpectedBase::Failure(ty) => self.add_node(
                node,
                ty,
                CheckedOperation::Outcome(CheckedOutcome::Failure(
                    payload.ok_or(BodyCheckInternalError::InvalidSyntax(node))?,
                )),
            )?,
        };
        self.materialize_injections(node, current, &injections)
    }

    fn materialize_injections(
        &mut self,
        node: NodeId,
        mut current: BodyNodeId,
        injections: &[OutcomeLayer],
    ) -> Result<BodyNodeId, BodyCheckError> {
        for layer in injections {
            let payload_type = self.node_type(current)?;
            let ty = self
                .types
                .intern(match layer {
                    OutcomeLayer::Optional => TypeKind::Optional(payload_type),
                    OutcomeLayer::Fallible => TypeKind::Fallible(payload_type),
                })
                .map_err(|_| BodyCheckInternalError::UnknownType(payload_type))?;
            current = self.add_node(
                node,
                ty,
                CheckedOperation::Outcome(CheckedOutcome::Inject {
                    layer: *layer,
                    payload: current,
                }),
            )?;
        }
        Ok(current)
    }

    pub(super) fn expected_error(&self, node: NodeId, error: ExpectedTypeError) -> BodyCheckError {
        match error {
            ExpectedTypeError::Mismatch { .. } => self
                .rule(BodyRule::TypeMismatch, node)
                .unwrap_or_else(BodyCheckError::Internal),
            ExpectedTypeError::UnknownType(ty) => BodyCheckInternalError::UnknownType(ty).into(),
        }
    }
}

fn borrowed_type(
    types: &nocter_model::TypeStore,
    ty: TypeId,
) -> Option<(BorrowCapability, TypeId)> {
    match types.get(ty)? {
        TypeKind::Borrow {
            capability,
            referent,
        } => Some((*capability, *referent)),
        _ => None,
    }
}
