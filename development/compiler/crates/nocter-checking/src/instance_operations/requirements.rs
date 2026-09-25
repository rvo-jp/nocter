use nocter_model::{BorrowCapability, CallableGuarantees, TypeId};

use super::comparison::ComparisonCandidateImplementation;
use super::selection::{
    InstanceOperationSelector, InstanceSelectionError, borrow_result, builtin_index_result,
    retain_direct_candidates,
};
use crate::copyability::CopyProofs;
use crate::interface_implementation::{
    CallableProofContext, predicate_implies, proves_predicate, substitute_predicate,
};
use crate::type_relations::TypeSubstitution;
use crate::{
    CheckedPredicate, CheckedRequirement, ComparisonOperation, Copyability, StaticDispatch,
    StaticSelection,
};

impl InstanceOperationSelector<'_> {
    pub(crate) fn requirements_hold(
        &mut self,
        requirements: &[CheckedRequirement],
        substitution: &TypeSubstitution,
    ) -> Result<bool, InstanceSelectionError> {
        for requirement in requirements {
            let predicate =
                substitute_predicate(self.types, substitution, requirement.predicate())?;
            if !self.proves_requirement(&predicate)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn proves_requirement(
        &mut self,
        predicate: &CheckedPredicate,
    ) -> Result<bool, InstanceSelectionError> {
        if self.explicit_facts_imply(predicate)? {
            return Ok(true);
        }
        if !self.active.insert(predicate.clone()) {
            return Ok(false);
        }
        let proven = match predicate {
            CheckedPredicate::Copy(ty) => {
                let proofs = CopyProofs::from_predicates(
                    self.types,
                    self.proof_assumptions()
                        .iter()
                        .map(CheckedRequirement::predicate)
                        .chain(
                            self.body_assumptions()
                                .iter()
                                .map(crate::body_check::BodyRequirement::predicate),
                        )
                        .chain(self.intrinsic_facts.iter()),
                );
                self.copyabilities
                    .classify_with_proofs(self.graph, self.types, *ty, &proofs)
                    .map_err(InstanceSelectionError::Copyability)?
                    == Copyability::Copy
            }
            CheckedPredicate::Index {
                capability,
                container,
                index,
                result,
                guarantees,
            } => self.proves_index(*container, *index, *result, *capability, *guarantees)?,
            CheckedPredicate::Coercion {
                source,
                target,
                guarantees,
            } => self.proves_coercion(*source, *target, *guarantees)?,
            CheckedPredicate::Equality {
                operand,
                guarantees,
            } => self.proves_comparison(*operand, ComparisonOperation::Equal, *guarantees)?,
            CheckedPredicate::Ordering {
                operand,
                guarantees,
            } => self.proves_comparison(*operand, ComparisonOperation::Less, *guarantees)?,
            CheckedPredicate::Expansion {
                capability,
                source,
                result,
                guarantees,
            } => {
                let mut matching = 0;
                for candidate in self.select_expansions(*source, *capability)? {
                    if candidate.result() == *result
                        && self.selection_satisfies(candidate.selection(), *guarantees)?
                    {
                        matching += 1;
                    }
                }
                matching == 1
            }
            _ => {
                if self.uses_body_evidence() {
                    proves_predicate(
                        match self.closures {
                            Some(closures) => {
                                CallableProofContext::executable(self.graph, closures)
                            }
                            None => CallableProofContext::declarations(self.graph),
                        },
                        self.types,
                        self.interface_implementations,
                        self.body_assumptions(),
                        self.intrinsic_facts,
                        predicate,
                    )?
                } else {
                    proves_predicate(
                        match self.closures {
                            Some(closures) => {
                                CallableProofContext::executable(self.graph, closures)
                            }
                            None => CallableProofContext::declarations(self.graph),
                        },
                        self.types,
                        self.interface_implementations,
                        self.proof_assumptions(),
                        self.intrinsic_facts,
                        predicate,
                    )?
                }
            }
        };
        self.active.remove(predicate);
        Ok(proven)
    }

    /// Tests already-admitted facts with the same directional weakening rule used by the
    /// interface prover. Keeping this rule shared prevents an operation contract such as
    /// `noalloc notrap` from becoming unusable where only `notrap` is required.
    fn explicit_facts_imply(
        &mut self,
        expected: &CheckedPredicate,
    ) -> Result<bool, InstanceSelectionError> {
        let assumptions = self
            .proof_assumptions()
            .iter()
            .map(CheckedRequirement::predicate)
            .chain(
                self.body_assumptions()
                    .iter()
                    .map(crate::body_check::BodyRequirement::predicate),
            )
            .chain(self.intrinsic_facts.iter())
            .cloned()
            .collect::<Vec<_>>();
        for actual in &assumptions {
            if predicate_implies(self.types, actual, expected)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn proves_index(
        &mut self,
        container: TypeId,
        index: TypeId,
        result: TypeId,
        capability: BorrowCapability,
        guarantees: CallableGuarantees,
    ) -> Result<bool, InstanceSelectionError> {
        let (result_capability, referent) = borrow_result(self.types, result)
            .ok_or(InstanceSelectionError::InvalidStructuralIndex)?;
        if result_capability != capability {
            return Err(InstanceSelectionError::InvalidStructuralIndex);
        }
        if let Some(builtin_result) = builtin_index_result(self.types, container, capability) {
            return Ok(
                index == self.types.builtin(nocter_model::BuiltinType::Usize)
                    && referent == builtin_result
                    && CallableGuarantees::no_allocation().can_weaken_to(guarantees),
            );
        }
        let mut candidates = self.select_index_operations(container, capability)?;
        candidates.extend(self.select_coerced_index_operations(container, capability)?);
        retain_direct_candidates(&mut candidates);
        let mut matching = 0;
        for candidate in candidates {
            let operation_satisfies = match candidate.operation() {
                Some(selection) => self.selection_satisfies(selection, guarantees)?,
                None => CallableGuarantees::no_allocation().can_weaken_to(guarantees),
            };
            if candidate.index() == index
                && candidate.result() == referent
                && operation_satisfies
                && self.optional_selection_satisfies(candidate.receiver_coercion(), guarantees)?
            {
                matching += 1;
            }
        }
        Ok(matching == 1)
    }

    fn proves_coercion(
        &mut self,
        source: TypeId,
        target: TypeId,
        guarantees: CallableGuarantees,
    ) -> Result<bool, InstanceSelectionError> {
        let Some((source_capability, source)) = borrow_result(self.types, source) else {
            return Ok(false);
        };
        let Some((target_capability, target)) = borrow_result(self.types, target) else {
            return Ok(false);
        };
        if source == target
            && source_capability == BorrowCapability::ReadWrite
            && target_capability == BorrowCapability::Readonly
        {
            return Ok(CallableGuarantees::no_allocation()
                .no_trap()
                .can_weaken_to(guarantees));
        }
        if source_capability != target_capability
            && (source_capability != BorrowCapability::ReadWrite
                || target_capability != BorrowCapability::Readonly)
        {
            return Ok(false);
        }
        let candidates =
            self.select_borrow_coercions(source, source_capability, target_capability)?;
        let mut matching = 0;
        for candidate in candidates {
            if candidate.target == target
                && self.selection_satisfies(candidate.selection(), guarantees)?
            {
                matching += 1;
            }
        }
        Ok(matching == 1)
    }

    fn proves_comparison(
        &mut self,
        ty: TypeId,
        operation: ComparisonOperation,
        guarantees: CallableGuarantees,
    ) -> Result<bool, InstanceSelectionError> {
        let mut matching = 0;
        for candidate in self.select_comparison_operations(ty, ty, operation)? {
            let operation_satisfies = match candidate.implementation() {
                ComparisonCandidateImplementation::Primitive => CallableGuarantees::no_allocation()
                    .no_trap()
                    .can_weaken_to(guarantees),
                ComparisonCandidateImplementation::Selected(selection) => {
                    self.selection_satisfies(selection, guarantees)?
                }
            };
            if operation_satisfies
                && self.optional_selection_satisfies(candidate.receiver_coercion(), guarantees)?
                && self.optional_selection_satisfies(candidate.argument_coercion(), guarantees)?
            {
                matching += 1;
            }
        }
        Ok(matching == 1)
    }

    fn optional_selection_satisfies(
        &self,
        selection: Option<&StaticSelection>,
        required: CallableGuarantees,
    ) -> Result<bool, InstanceSelectionError> {
        selection.map_or(Ok(true), |selection| {
            self.selection_satisfies(selection, required)
        })
    }

    fn selection_satisfies(
        &self,
        selection: &StaticSelection,
        required: CallableGuarantees,
    ) -> Result<bool, InstanceSelectionError> {
        let actual = match selection.dispatch() {
            StaticDispatch::Direct(callable)
            | StaticDispatch::InterfaceMethod {
                method: callable, ..
            }
            | StaticDispatch::InterfaceSelfMethod {
                method: callable, ..
            }
            | StaticDispatch::InterfaceDefault {
                method: callable, ..
            }
            | StaticDispatch::OpaqueMethod {
                method: callable, ..
            } => self
                .graph
                .declarations()
                .callables()
                .get(callable)
                .map(nocter_declarations::CallableDeclaration::guarantees)
                .ok_or(InstanceSelectionError::MissingCallable(callable))?,
            StaticDispatch::StructuralRequirement { evidence } => self
                .body_assumptions()
                .iter()
                .find(|assumption| assumption.evidence() == evidence)
                .and_then(|assumption| assumption.predicate().structural_guarantees())
                .ok_or(InstanceSelectionError::MissingCapabilityEvidence(evidence))?,
        };
        Ok(actual.can_weaken_to(required))
    }
}
