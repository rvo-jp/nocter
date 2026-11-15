//! One source-bound instance selection shared by metadata consumers, never executable permission.
use super::Unit;
use crate::value_access::{
    InstanceControl, InstantiationFailure, ResourceDataFailure, ResourceDataProjection,
    ValueSourceId, ValueSourceSelection, flow, results,
};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{CodeInstanceKey, DataInstanceKey};
use nocter_model::{TypeId, ValueApplication};
use std::sync::Arc;

#[derive(Clone)]
pub struct InstanceMetadata {
    unit: Arc<Unit>,
    control: InstanceControl,
}
impl std::fmt::Debug for InstanceMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceMetadata")
            .field("control", &self.control)
            .finish_non_exhaustive()
    }
}
impl Unit {
    /// Bind this exact Unit and canonical frame to one retained original-control selection.
    /// # Errors
    /// Rejects missing bodies, inconsistent frame metadata and exhausted work.
    pub fn instance_metadata(
        self: &Arc<Self>,
        frame: &CodeInstanceKey,
        work: &mut WorkBudget,
    ) -> Result<InstanceMetadata, InstantiationFailure> {
        Ok(InstanceMetadata {
            control: self.instantiate_control(frame, work)?,
            unit: self.clone(),
        })
    }
}
impl InstanceMetadata {
    #[must_use]
    pub fn unit(&self) -> &Unit {
        &self.unit
    }
    #[must_use]
    pub const fn control(&self) -> &InstanceControl {
        &self.control
    }
    fn query<'work>(&self, work: &'work mut WorkBudget) -> results::InstantiationQuery<'_, 'work> {
        results::InstantiationQuery::with_control(
            &self.unit.program,
            &self.unit.result_evidence,
            work,
            &self.control,
        )
    }
    /// Resolve an original call under this exact Unit and frame's selected incoming edges.
    /// # Errors
    /// Rejects foreign/missing calls, ambiguous metadata and exhausted work.
    pub fn invocation(
        &self,
        site: ValueApplication,
        work: &mut WorkBudget,
    ) -> Result<CodeInstanceKey, InstantiationFailure> {
        self.query(work).call(self.control.frame(), site)
    }
    /// Select contextual data without granting an initialized value, access or storage.
    /// # Errors
    /// Rejects missing sources, incompatible/ambiguous contexts and exhausted work.
    pub fn value_data(
        &self,
        value: ValueSourceId,
        data: TypeId,
        work: &mut WorkBudget,
    ) -> Result<DataInstanceKey, InstantiationFailure> {
        self.query(work).data(self.control.frame(), value, data)
    }
    /// Select an original stored definition, independently of resource phases and availability.
    /// # Errors
    /// Rejects invalid boundaries, ambiguous contexts and exhausted work.
    pub fn stored_data(
        &self,
        selected: &ValueSourceSelection,
        data: TypeId,
        work: &mut WorkBudget,
    ) -> Result<DataInstanceKey, InstantiationFailure> {
        self.query(work)
            .stored(self.control.frame(), selected, data)
    }
    /// Project resource metadata while preserving original retirement and receipt order.
    /// # Errors
    /// Rejects missing operations, inconsistent metadata and exhausted work.
    pub fn resource_data<'a, 'work>(
        &'a self,
        at: flow::Location,
        work: &'work mut WorkBudget,
    ) -> Result<ResourceDataProjection<'a, 'work>, ResourceDataFailure> {
        ResourceDataProjection::new(
            &self.unit.program,
            &self.unit.result_evidence,
            self.unit.code_body(self.control.frame().code()),
            self.control.frame(),
            at,
            Some(&self.control),
            work,
        )
    }
}
