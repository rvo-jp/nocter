use std::collections::HashMap;
use std::sync::Arc;

use nocter_model::BodyNodeId;

use crate::{CompileTimeCallTarget, CompileTimeCallablePlan, CompileTimeOperation};

/// Canonical program-owned compile-time plan authority.
///
/// Only closed callable specializations that are reachable from a compile-time root occupy the
/// table. Consumers cannot trigger projection, reopen generic substitution, or infer a second plan
/// from checked bodies.
#[derive(Clone, Debug, Default)]
pub struct CompileTimePlanTable {
    plans: HashMap<CompileTimeCallTarget, Arc<CompileTimeCallablePlan>>,
}

impl CompileTimePlanTable {
    /// Closes one queried specialization set after proving every direct call has an executable
    /// specialization in the same set.
    ///
    /// # Errors
    ///
    /// Returns the caller, operation node, and unavailable target without publishing a partial
    /// authority.
    pub fn new(
        plans: HashMap<CompileTimeCallTarget, Arc<CompileTimeCallablePlan>>,
    ) -> Result<Self, InvalidCompileTimePlanTable> {
        let mut callers = plans.keys().collect::<Vec<_>>();
        callers.sort_unstable();
        for caller in callers {
            let plan = &plans[caller];
            for (node, operation) in plan.nodes().iter() {
                let CompileTimeOperation::Call { target, .. } = operation.operation() else {
                    continue;
                };
                if !plans.contains_key(target) {
                    return Err(InvalidCompileTimePlanTable {
                        caller: caller.clone(),
                        node,
                        target: target.clone(),
                    });
                }
            }
        }
        Ok(Self { plans })
    }

    #[must_use]
    pub fn get(&self, target: &CompileTimeCallTarget) -> Option<&CompileTimeCallablePlan> {
        self.plans.get(target).map(AsRef::as_ref)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.plans.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidCompileTimePlanTable {
    caller: CompileTimeCallTarget,
    node: BodyNodeId,
    target: CompileTimeCallTarget,
}

impl InvalidCompileTimePlanTable {
    #[must_use]
    pub const fn caller(&self) -> &CompileTimeCallTarget {
        &self.caller
    }

    #[must_use]
    pub const fn node(&self) -> BodyNodeId {
        self.node
    }

    #[must_use]
    pub const fn target(&self) -> &CompileTimeCallTarget {
        &self.target
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use nocter_model::{Arena, ArenaBuilder};

    use super::CompileTimePlanTable;
    use crate::{
        CompileTimeCallTarget, CompileTimeCallablePlan, CompileTimeNode, CompileTimeOperation,
        CompileTimeValueType,
    };

    #[test]
    fn table_rejects_a_direct_call_without_an_executable_target_plan() {
        let mut identities = ArenaBuilder::new();
        let caller = identities.insert(());
        let target = identities.insert(());
        let mut nodes = ArenaBuilder::new();
        let call = nodes.insert(CompileTimeNode::new(
            CompileTimeValueType::Void,
            CompileTimeOperation::Call {
                target: CompileTimeCallTarget::new(target, []).unwrap(),
                receiver: None,
                arguments: Box::new([]),
            },
        ));
        let caller_plan =
            CompileTimeCallablePlan::new([], Arena::default(), nodes.finish(), call).unwrap();
        let caller_target = CompileTimeCallTarget::new(caller, []).unwrap();
        let mut plans = HashMap::new();
        plans.insert(caller_target.clone(), Arc::new(caller_plan));

        let error = CompileTimePlanTable::new(plans).unwrap_err();

        assert_eq!(error.caller(), &caller_target);
        assert_eq!(error.node(), call);
        assert_eq!(error.target().callable(), target);
    }
}
