use nocter_runtime_contract::PrimitiveRole;

use crate::{Arm64FunctionId, Arm64ProgramBuilder};

/// Shared native lifecycle entries for compiler-owned fixed-cardinality wait computations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64AsyncInterestLifecycleTargets {
    resume: Arm64FunctionId,
    cancel: Arm64FunctionId,
    consume: Arm64FunctionId,
    interest_count: u8,
}

/// Native lifecycle entries for one two-child structured composition computation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64AsyncPairTargets {
    constructor: Arm64FunctionId,
    resume: Arm64FunctionId,
    cancel: Arm64FunctionId,
    consume: Arm64FunctionId,
}

impl Arm64AsyncPairTargets {
    #[must_use]
    pub const fn constructor(self) -> Arm64FunctionId {
        self.constructor
    }

    #[must_use]
    pub const fn resume(self) -> Arm64FunctionId {
        self.resume
    }

    #[must_use]
    pub const fn cancel(self) -> Arm64FunctionId {
        self.cancel
    }

    #[must_use]
    pub const fn consume(self) -> Arm64FunctionId {
        self.consume
    }
}

impl Arm64AsyncInterestLifecycleTargets {
    #[must_use]
    pub const fn resume(self) -> Arm64FunctionId {
        self.resume
    }

    #[must_use]
    pub const fn cancel(self) -> Arm64FunctionId {
        self.cancel
    }

    #[must_use]
    pub const fn consume(self) -> Arm64FunctionId {
        self.consume
    }

    #[must_use]
    pub const fn interest_count(self) -> u64 {
        self.interest_count as u64
    }
}

/// Native helper identities selected once from the machine program's primitive dependencies.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Arm64AsyncPrimitiveTargets {
    descriptor_readiness: Option<Arm64FunctionId>,
    descriptor_readiness_or_deadline: Option<Arm64FunctionId>,
    monotonic_deadline: Option<Arm64FunctionId>,
    process_completion: Option<Arm64FunctionId>,
    single_interest_lifecycle: Option<Arm64AsyncInterestLifecycleTargets>,
    dual_interest_lifecycle: Option<Arm64AsyncInterestLifecycleTargets>,
    task_join: Option<Arm64AsyncPairTargets>,
    task_race: Option<Arm64AsyncPairTargets>,
}

impl Arm64AsyncPrimitiveTargets {
    pub(crate) fn declare(
        roles: &std::collections::BTreeSet<PrimitiveRole>,
        builder: &mut Arm64ProgramBuilder,
    ) -> Self {
        let descriptor_readiness = roles.contains(&PrimitiveRole::DescriptorReadiness);
        let descriptor_readiness_or_deadline =
            roles.contains(&PrimitiveRole::DescriptorReadinessOrDeadline);
        let monotonic_deadline = roles.contains(&PrimitiveRole::MonotonicDeadline);
        let process_completion = roles.contains(&PrimitiveRole::ProcessCompletion);
        let task_join = roles.contains(&PrimitiveRole::TaskJoin);
        let task_race = roles.contains(&PrimitiveRole::TaskRace);
        let single_interest_lifecycle =
            (descriptor_readiness || monotonic_deadline || process_completion)
                .then(|| declare_lifecycle(builder, 1));
        let dual_interest_lifecycle =
            descriptor_readiness_or_deadline.then(|| declare_lifecycle(builder, 2));
        Self {
            descriptor_readiness: descriptor_readiness.then(|| builder.declare_function()),
            descriptor_readiness_or_deadline: descriptor_readiness_or_deadline
                .then(|| builder.declare_function()),
            monotonic_deadline: monotonic_deadline.then(|| builder.declare_function()),
            process_completion: process_completion.then(|| builder.declare_function()),
            single_interest_lifecycle,
            dual_interest_lifecycle,
            task_join: task_join.then(|| declare_pair(builder)),
            task_race: task_race.then(|| declare_pair(builder)),
        }
    }

    #[must_use]
    pub const fn descriptor_readiness(self) -> Option<Arm64FunctionId> {
        self.descriptor_readiness
    }

    #[must_use]
    pub const fn descriptor_readiness_or_deadline(self) -> Option<Arm64FunctionId> {
        self.descriptor_readiness_or_deadline
    }

    #[must_use]
    pub const fn monotonic_deadline(self) -> Option<Arm64FunctionId> {
        self.monotonic_deadline
    }

    #[must_use]
    pub const fn process_completion(self) -> Option<Arm64FunctionId> {
        self.process_completion
    }

    #[must_use]
    pub const fn single_interest_lifecycle(self) -> Option<Arm64AsyncInterestLifecycleTargets> {
        self.single_interest_lifecycle
    }

    #[must_use]
    pub const fn dual_interest_lifecycle(self) -> Option<Arm64AsyncInterestLifecycleTargets> {
        self.dual_interest_lifecycle
    }

    #[must_use]
    pub const fn task_join(self) -> Option<Arm64AsyncPairTargets> {
        self.task_join
    }

    #[must_use]
    pub const fn task_race(self) -> Option<Arm64AsyncPairTargets> {
        self.task_race
    }
}

fn declare_pair(builder: &mut Arm64ProgramBuilder) -> Arm64AsyncPairTargets {
    Arm64AsyncPairTargets {
        constructor: builder.declare_function(),
        resume: builder.declare_function(),
        cancel: builder.declare_function(),
        consume: builder.declare_function(),
    }
}

fn declare_lifecycle(
    builder: &mut Arm64ProgramBuilder,
    interest_count: u8,
) -> Arm64AsyncInterestLifecycleTargets {
    Arm64AsyncInterestLifecycleTargets {
        resume: builder.declare_function(),
        cancel: builder.declare_function(),
        consume: builder.declare_function(),
        interest_count,
    }
}
