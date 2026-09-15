use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use nocter_model::{BodyNodeId, ConstantId};

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
                let CompileTimeOperation::Call {
                    target,
                    receiver,
                    arguments,
                } = operation.operation()
                else {
                    continue;
                };
                let Some(target_plan) = plans.get(target) else {
                    return Err(InvalidCompileTimePlanTable {
                        caller: caller.clone(),
                        node,
                        target: target.clone(),
                        rule: InvalidCompileTimePlanRule::MissingTarget,
                    });
                };
                let inputs = receiver
                    .iter()
                    .chain(arguments.iter())
                    .copied()
                    .collect::<Vec<_>>();
                if inputs.len() != target_plan.parameters().len() {
                    return Err(InvalidCompileTimePlanTable {
                        caller: caller.clone(),
                        node,
                        target: target.clone(),
                        rule: InvalidCompileTimePlanRule::ArgumentCount,
                    });
                }
                if inputs
                    .iter()
                    .zip(target_plan.parameters())
                    .any(|(input, parameter)| {
                        plan.nodes().get(*input).map(crate::CompileTimeNode::ty)
                            != Some(parameter.ty())
                    })
                {
                    return Err(InvalidCompileTimePlanTable {
                        caller: caller.clone(),
                        node,
                        target: target.clone(),
                        rule: InvalidCompileTimePlanRule::ArgumentType,
                    });
                }
                if operation.ty() != target_plan.result() {
                    return Err(InvalidCompileTimePlanTable {
                        caller: caller.clone(),
                        node,
                        target: target.clone(),
                        rule: InvalidCompileTimePlanRule::ResultType,
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

    /// Closes the declared-constant inputs reachable from `root` through the frozen call graph.
    ///
    /// # Errors
    ///
    /// Returns the unavailable selected call target when `root` is not closed by this table.
    pub fn constant_dependency_closure(
        &self,
        root: &CompileTimeCallablePlan,
    ) -> Result<Box<[ConstantId]>, MissingCompileTimePlan> {
        let mut constants = BTreeSet::new();
        let mut visited = HashSet::new();
        let mut pending = root.call_dependencies().to_vec();
        constants.extend(root.constant_dependencies().iter().copied());
        while let Some(target) = pending.pop() {
            if !visited.insert(target.clone()) {
                continue;
            }
            let plan = self.get(&target).ok_or(MissingCompileTimePlan { target })?;
            constants.extend(plan.constant_dependencies().iter().copied());
            pending.extend(plan.call_dependencies().iter().cloned());
        }
        Ok(constants.into_iter().collect())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingCompileTimePlan {
    target: CompileTimeCallTarget,
}

impl MissingCompileTimePlan {
    #[must_use]
    pub const fn target(&self) -> &CompileTimeCallTarget {
        &self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidCompileTimePlanTable {
    caller: CompileTimeCallTarget,
    node: BodyNodeId,
    target: CompileTimeCallTarget,
    rule: InvalidCompileTimePlanRule,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidCompileTimePlanRule {
    MissingTarget,
    ArgumentCount,
    ArgumentType,
    ResultType,
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

    #[must_use]
    pub const fn rule(&self) -> InvalidCompileTimePlanRule {
        self.rule
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use nocter_model::{Arena, ArenaBuilder};

    use super::{CompileTimePlanTable, InvalidCompileTimePlanRule};
    use crate::{
        CompileTimeCallTarget, CompileTimeCallablePlan, CompileTimeNode, CompileTimeOperation,
        CompileTimeParameter, CompileTimeValueType,
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
        let caller_plan = CompileTimeCallablePlan::new(
            Vec::<CompileTimeParameter<CompileTimeValueType>>::new(),
            CompileTimeValueType::Void,
            Arena::default(),
            nodes.finish(),
            call,
        )
        .unwrap();
        let caller_target = CompileTimeCallTarget::new(caller, []).unwrap();
        let mut plans = HashMap::new();
        plans.insert(caller_target.clone(), Arc::new(caller_plan));

        let error = CompileTimePlanTable::new(plans).unwrap_err();

        assert_eq!(error.caller(), &caller_target);
        assert_eq!(error.node(), call);
        assert_eq!(error.target().callable(), target);
        assert_eq!(error.rule(), InvalidCompileTimePlanRule::MissingTarget);
    }

    #[test]
    fn table_rejects_a_call_whose_result_shape_disagrees_with_its_target() {
        let mut identities = ArenaBuilder::new();
        let caller = identities.insert(());
        let target = identities.insert(());
        let mut target_nodes = ArenaBuilder::new();
        let target_root = target_nodes.insert(CompileTimeNode::new(
            CompileTimeValueType::Void,
            CompileTimeOperation::Complete,
        ));
        let target_plan = CompileTimeCallablePlan::new(
            Vec::<CompileTimeParameter<CompileTimeValueType>>::new(),
            CompileTimeValueType::Void,
            Arena::default(),
            target_nodes.finish(),
            target_root,
        )
        .unwrap();
        let target_identity = CompileTimeCallTarget::new(target, []).unwrap();
        let mut caller_nodes = ArenaBuilder::new();
        let call = caller_nodes.insert(CompileTimeNode::new(
            CompileTimeValueType::Scalar(crate::ConstantScalarType::Bool),
            CompileTimeOperation::Call {
                target: target_identity.clone(),
                receiver: None,
                arguments: Box::new([]),
            },
        ));
        let caller_plan = CompileTimeCallablePlan::new(
            Vec::<CompileTimeParameter<CompileTimeValueType>>::new(),
            CompileTimeValueType::Scalar(crate::ConstantScalarType::Bool),
            Arena::default(),
            caller_nodes.finish(),
            call,
        )
        .unwrap();
        let caller_identity = CompileTimeCallTarget::new(caller, []).unwrap();

        let error = CompileTimePlanTable::new(HashMap::from([
            (caller_identity, Arc::new(caller_plan)),
            (target_identity, Arc::new(target_plan)),
        ]))
        .unwrap_err();

        assert_eq!(error.rule(), InvalidCompileTimePlanRule::ResultType);
        assert_eq!(error.node(), call);
    }
}
