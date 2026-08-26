use nocter_declarations::{ExpansionCapability, InterfaceApplication};
use nocter_model::{BorrowCapability, CallableId, TypeId};

use super::comparison::ComparisonOperationCandidate;
use super::expansion::ExpansionCandidate;
use super::methods::MethodCandidate;
use super::selection::{CoercionCandidate, IndexOperationCandidate, retain_direct_candidates};
use super::{InstanceOperationSelector, InstanceSelectionContext};
use crate::{CheckedProgram, ComparisonOperation, InstanceSelectionError};

/// Narrow specialization-time access to evidence accepted by semantic checking.
///
/// This authority has no lookup by spelling and receives no source namespace. Every operation is
/// anchored by a checked interface application or structural requirement shape and is restricted
/// to the already validated interface implementation and instance-operation tables.
pub(crate) struct ConcreteEvidenceAuthority<'authority> {
    selector: InstanceOperationSelector<'authority>,
}

impl<'authority> ConcreteEvidenceAuthority<'authority> {
    pub(crate) fn new(
        program: &'authority CheckedProgram,
        types: &'authority mut nocter_model::TypeTransaction,
        copyabilities: &'authority mut crate::copyability::CopyabilityTransaction,
    ) -> Self {
        Self {
            selector: InstanceOperationSelector::new(
                InstanceSelectionContext::for_concrete_evidence(
                    program.graph(),
                    program.interface_implementations(),
                    program.instance_operations(),
                ),
                types,
                copyabilities,
            ),
        }
    }

    pub(crate) fn interface_method(
        &mut self,
        subject: TypeId,
        application: &InterfaceApplication,
        method: CallableId,
    ) -> Result<Vec<MethodCandidate>, InstanceSelectionError> {
        self.selector
            .select_interface_implementation_method_for_application(subject, application, method)
    }

    pub(crate) fn comparison(
        &mut self,
        subject: TypeId,
        operand: TypeId,
        operation: ComparisonOperation,
    ) -> Result<Vec<ComparisonOperationCandidate>, InstanceSelectionError> {
        self.selector
            .select_comparison_operations(subject, operand, operation)
    }

    pub(crate) fn index(
        &mut self,
        container: TypeId,
        capability: BorrowCapability,
        index: TypeId,
        result: TypeId,
    ) -> Result<Vec<IndexOperationCandidate>, InstanceSelectionError> {
        let mut candidates = self
            .selector
            .select_index_operations(container, capability)?;
        candidates.extend(
            self.selector
                .select_coerced_index_operations(container, capability)?,
        );
        retain_direct_candidates(&mut candidates);
        candidates.retain(|candidate| candidate.index() == index && candidate.result() == result);
        Ok(candidates)
    }

    pub(crate) fn coercion(
        &mut self,
        source: TypeId,
        source_capability: BorrowCapability,
        target_capability: BorrowCapability,
        target: TypeId,
    ) -> Result<Vec<CoercionCandidate>, InstanceSelectionError> {
        Ok(self
            .selector
            .select_borrow_coercions(source, source_capability, target_capability)?
            .into_iter()
            .filter(|candidate| candidate.target() == target)
            .collect())
    }

    pub(crate) fn expansion(
        &mut self,
        source: TypeId,
        capability: ExpansionCapability,
        result: TypeId,
    ) -> Result<Vec<ExpansionCandidate>, InstanceSelectionError> {
        Ok(self
            .selector
            .select_expansions(source, capability)?
            .into_iter()
            .filter(|candidate| candidate.result() == result)
            .collect())
    }
}
