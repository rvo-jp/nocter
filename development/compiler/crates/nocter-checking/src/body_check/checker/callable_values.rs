use nocter_declarations::StructuralCapability;
use nocter_model::{BodyNodeId, CallableCapability, TypeId};
use nocter_syntax::NodeId;

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::direct_nodes;
use crate::{
    CallTarget, CheckedCall, CheckedOperation, CheckedPredicate, GenericArguments, PlaceAccess,
    StaticDispatch, StaticSelection,
};

impl BodyChecker<'_, '_> {
    pub(super) fn check_callable_value_call(
        &mut self,
        node: NodeId,
        reference: NodeId,
        suffix: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let place = self.named_place(reference)?;
        let (requirement, contract) = self.callable_contract(place.ty, node)?;
        let argument_syntax = direct_nodes(self.tree(), suffix);
        if argument_syntax.len() != contract.parameters().len() {
            return Err(self.rule(BodyRule::InvalidCall, suffix)?);
        }
        if contract.capability() == CallableCapability::ReadWrite
            && !self.is_writable_place(place.id)?
        {
            return Err(self.rule(BodyRule::InvalidCall, reference)?);
        }
        if contract.capability() == CallableCapability::Owned && place.access != PlaceAccess::Owned
        {
            return Err(self.rule(BodyRule::InvalidCall, reference)?);
        }

        let value = self.add_node(reference, place.ty, CheckedOperation::Place(place.id))?;
        let arguments = argument_syntax
            .into_iter()
            .zip(contract.parameters().iter().copied())
            .map(|(argument, parameter)| self.check_expression(argument, Some(parameter)))
            .collect::<Result<Vec<_>, _>>()?;
        let result = contract.result();
        let call = self.add_node(
            node,
            result,
            CheckedOperation::Call(CheckedCall::new(
                CallTarget::CallableValue {
                    value,
                    capability: contract.capability(),
                    dispatch: StaticSelection::new(
                        StaticDispatch::StructuralRequirement(requirement),
                        GenericArguments::default(),
                    ),
                },
                None,
                arguments,
            )),
        )?;
        expected.map_or(Ok(call), |expected| {
            self.apply_expected(node, call, expected)
        })
    }

    fn callable_contract(
        &self,
        subject: TypeId,
        node: NodeId,
    ) -> Result<(nocter_model::RequirementId, nocter_model::CallableContract), BodyCheckError> {
        let mut candidates = self.assumptions.iter().filter_map(|requirement| {
            let CheckedPredicate::Capability {
                subject: required_subject,
                capability: StructuralCapability::Callable(contract),
            } = requirement.predicate()
            else {
                return None;
            };
            (*required_subject == subject).then(|| (requirement.declaration(), contract.clone()))
        });
        let Some(selected) = candidates.next() else {
            return Err(self.rule(BodyRule::InvalidCall, node)?);
        };
        if candidates.next().is_some() {
            return Err(BodyCheckInternalError::CallContractSelection.into());
        }
        Ok(selected)
    }
}
