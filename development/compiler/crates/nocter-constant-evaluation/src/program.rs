use nocter_model::{Arena, BodyNodeId, CallableId};

use crate::{CompileTimeCallablePlan, CompileTimeOperation};

/// Canonical program-owned compile-time plan authority.
///
/// Every callable occupies its declaration identity slot. `None` means that the declaration has no
/// executable compile-time plan, either because it is runtime-only or because it is a bodyless
/// interface contract. Consumers cannot trigger projection or infer a second plan from checked
/// bodies.
#[derive(Clone, Debug, Default)]
pub struct CompileTimePlanTable {
    plans: Arena<CallableId, Option<CompileTimeCallablePlan>>,
}

impl CompileTimePlanTable {
    /// Closes one canonical callable arena after proving every direct call has an executable plan.
    ///
    /// # Errors
    ///
    /// Returns the caller, operation node, and unavailable target without publishing a partial
    /// authority.
    pub fn new(
        plans: Arena<CallableId, Option<CompileTimeCallablePlan>>,
    ) -> Result<Self, InvalidCompileTimePlanTable> {
        for (caller, plan) in plans.iter() {
            let Some(plan) = plan else {
                continue;
            };
            for (node, operation) in plan.nodes().iter() {
                let CompileTimeOperation::Call { target, .. } = operation.operation() else {
                    continue;
                };
                if plans
                    .get(target.callable())
                    .and_then(Option::as_ref)
                    .is_none()
                {
                    return Err(InvalidCompileTimePlanTable {
                        caller,
                        node,
                        target: target.callable(),
                    });
                }
            }
        }
        Ok(Self { plans })
    }

    #[must_use]
    pub fn get(&self, callable: CallableId) -> Option<&CompileTimeCallablePlan> {
        self.plans.get(callable).and_then(Option::as_ref)
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.plans.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidCompileTimePlanTable {
    caller: CallableId,
    node: BodyNodeId,
    target: CallableId,
}

impl InvalidCompileTimePlanTable {
    #[must_use]
    pub const fn caller(self) -> CallableId {
        self.caller
    }

    #[must_use]
    pub const fn node(self) -> BodyNodeId {
        self.node
    }

    #[must_use]
    pub const fn target(self) -> CallableId {
        self.target
    }
}

#[cfg(test)]
mod tests {
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
        let mut plans = ArenaBuilder::new();
        assert_eq!(plans.insert(Some(caller_plan)), caller);
        assert_eq!(plans.insert(None), target);

        let error = CompileTimePlanTable::new(plans.finish()).unwrap_err();

        assert_eq!(error.caller(), caller);
        assert_eq!(error.node(), call);
        assert_eq!(error.target(), target);
    }
}
