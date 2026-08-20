use std::collections::{BTreeMap, BTreeSet};

use nocter_checking::{DropSelection, GenericArguments, TypeSubstitution};

use super::{ExecutableClosureBuilder, collect_drops};
use crate::{
    ExecutableItemKey, ExecutablePrimitiveDependency, ExecutableProgramError, PrimitiveRole,
};

impl ExecutableClosureBuilder<'_> {
    pub(super) fn specialize_primitive_dependency(
        &mut self,
        role: PrimitiveRole,
        arguments: &GenericArguments,
        drops: &mut BTreeMap<DropSelection, ExecutableItemKey>,
    ) -> Result<ExecutablePrimitiveDependency, ExecutableProgramError> {
        if role != PrimitiveRole::DropValueAtPointer {
            return Ok(ExecutablePrimitiveDependency::None);
        }
        let [argument] = arguments.as_slice() else {
            return Err(ExecutableProgramError::InvalidPrimitiveDependency(role));
        };
        let subject = argument.ty();
        let plan = self
            .resolver
            .resolve_destruction(subject, &TypeSubstitution::default())?;
        if let Some(plan) = &plan {
            let mut selections = BTreeSet::new();
            collect_drops(plan, &mut selections);
            for selection in selections {
                self.record_drop(selection, drops)?;
            }
        }
        Ok(ExecutablePrimitiveDependency::Destruction {
            subject,
            plan: plan.map(Box::new),
        })
    }
}
