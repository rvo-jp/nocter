use nocter_declarations::StructuralCapability;
use nocter_model::{BodyNodeId, CallableCapability, ClosureId, TypeId, TypeKind};
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
        let selected = self.callable_contract(place.ty, node)?;
        let (capability, parameters, result) = match &selected {
            CallableValueContract::Closure(_, signature) => (
                signature.capability(),
                signature.parameters(),
                signature.result(),
            ),
            CallableValueContract::Structural(_, contract) => (
                contract.capability(),
                contract.parameters(),
                contract.result(),
            ),
        };
        let argument_syntax = direct_nodes(self.tree(), suffix);
        if argument_syntax.len() != parameters.len() {
            return Err(self.rule(BodyRule::InvalidCall, suffix)?);
        }
        if capability == CallableCapability::ReadWrite && !self.is_writable_place(place.id)? {
            return Err(self.rule(BodyRule::InvalidCall, reference)?);
        }
        if capability == CallableCapability::Owned && place.access != PlaceAccess::Owned {
            return Err(self.rule(BodyRule::InvalidCall, reference)?);
        }

        let value = self.add_node(reference, place.ty, CheckedOperation::Place(place.id))?;
        let arguments = argument_syntax
            .into_iter()
            .zip(parameters.iter().copied())
            .map(|(argument, parameter)| self.check_expression(argument, Some(parameter)))
            .collect::<Result<Vec<_>, _>>()?;
        let target = match selected {
            CallableValueContract::Closure(closure, _) => CallTarget::ClosureValue {
                value,
                closure,
                capability,
            },
            CallableValueContract::Structural(requirement, _) => CallTarget::CallableValue {
                value,
                capability,
                dispatch: StaticSelection::new(
                    StaticDispatch::StructuralRequirement(requirement),
                    GenericArguments::default(),
                ),
            },
        };
        let call = self.add_node(
            node,
            result,
            CheckedOperation::Call(CheckedCall::new(target, None, arguments)),
        )?;
        expected.map_or(Ok(call), |expected| {
            self.apply_expected(node, call, expected)
        })
    }

    fn callable_contract(
        &self,
        subject: TypeId,
        node: NodeId,
    ) -> Result<CallableValueContract, BodyCheckError> {
        if let Some(TypeKind::Closure(closure)) = self.types.get(subject) {
            let definition = self
                .closures
                .get(*closure)
                .ok_or(BodyCheckInternalError::MissingClosure(*closure))?;
            return Ok(CallableValueContract::Closure(
                *closure,
                definition.signature().clone(),
            ));
        }
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
        Ok(CallableValueContract::Structural(selected.0, selected.1))
    }
}

enum CallableValueContract {
    Closure(ClosureId, crate::ClosureSignature),
    Structural(nocter_model::RequirementId, nocter_model::CallableContract),
}
