use nocter_model::{
    AllocationGuarantee, Arena, CallableGuarantees, CallableId, ClosureId, DropId,
    NonblockingGuarantee,
};

/// Positive allocation fact inferred for one closed execution root.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum AllocationFact {
    /// No reachable operation in the checked root requests new storage.
    #[default]
    NoAllocation,
    /// At least one reachable operation may request new storage.
    MayAllocate,
}

impl AllocationFact {
    #[must_use]
    pub const fn may_allocate(self) -> bool {
        matches!(self, Self::MayAllocate)
    }
}

/// Positive synchronous-wait fact inferred for one closed execution root.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SynchronousWaitFact {
    /// No reachable operation synchronously waits for external progress.
    #[default]
    Nonblocking,
    /// At least one reachable operation may synchronously wait for external progress.
    MayBlock,
}

impl SynchronousWaitFact {
    #[must_use]
    pub const fn may_block(self) -> bool {
        matches!(self, Self::MayBlock)
    }
}

/// Inferred positive facts for one closed execution root.
///
/// This is implementation evidence, not an authored callable contract. An immediate callable root
/// describes invocation of its body. A deferred callable root describes driving its body after
/// invocation has already constructed the owning future. Checked calls retain that temporal split
/// explicitly and relation collection never infers it from a result type.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ExecutionFacts {
    allocation: AllocationFact,
    synchronous_wait: SynchronousWaitFact,
}

impl ExecutionFacts {
    #[must_use]
    pub const fn new(allocation: AllocationFact, synchronous_wait: SynchronousWaitFact) -> Self {
        Self {
            allocation,
            synchronous_wait,
        }
    }

    #[must_use]
    pub const fn allocation(self) -> AllocationFact {
        self.allocation
    }

    #[must_use]
    pub const fn synchronous_wait(self) -> SynchronousWaitFact {
        self.synchronous_wait
    }

    /// Facts introduced by an operation that requests storage directly.
    #[must_use]
    pub(crate) const fn allocation_request() -> Self {
        Self::new(
            AllocationFact::MayAllocate,
            SynchronousWaitFact::Nonblocking,
        )
    }

    /// Conservative implementation facts admitted by one explicit external contract.
    ///
    /// This constructor is not body inference. It is used only where the selected implementation
    /// is unavailable by design, including bodyless declarations and structural dispatch.
    #[must_use]
    pub(crate) const fn admitted_by(guarantees: CallableGuarantees) -> Self {
        Self::new(
            match guarantees.allocation() {
                AllocationGuarantee::Unspecified => AllocationFact::MayAllocate,
                AllocationGuarantee::NoAllocation => AllocationFact::NoAllocation,
            },
            match guarantees.nonblocking() {
                NonblockingGuarantee::Nonblocking => SynchronousWaitFact::Nonblocking,
                NonblockingGuarantee::Unspecified => SynchronousWaitFact::MayBlock,
            },
        )
    }

    pub(crate) fn include(&mut self, other: Self) -> bool {
        let next = Self {
            allocation: if self.allocation.may_allocate() || other.allocation.may_allocate() {
                AllocationFact::MayAllocate
            } else {
                AllocationFact::NoAllocation
            },
            synchronous_wait: if self.synchronous_wait.may_block()
                || other.synchronous_wait.may_block()
            {
                SynchronousWaitFact::MayBlock
            } else {
                SynchronousWaitFact::Nonblocking
            },
        };
        if *self == next {
            false
        } else {
            *self = next;
            true
        }
    }
}

/// Whole-program least-fixed-point execution facts.
#[derive(Clone, Debug)]
pub struct ExecutionFactTable {
    callables: Arena<CallableId, ExecutionFacts>,
    closures: Arena<ClosureId, ExecutionFacts>,
    drops: Arena<DropId, ExecutionFacts>,
}

impl ExecutionFactTable {
    pub(super) const fn new(
        callables: Arena<CallableId, ExecutionFacts>,
        closures: Arena<ClosureId, ExecutionFacts>,
        drops: Arena<DropId, ExecutionFacts>,
    ) -> Self {
        Self {
            callables,
            closures,
            drops,
        }
    }

    #[must_use]
    pub fn callable(&self, callable: CallableId) -> Option<ExecutionFacts> {
        self.callables.get(callable).copied()
    }

    #[must_use]
    pub fn closure(&self, closure: ClosureId) -> Option<ExecutionFacts> {
        self.closures.get(closure).copied()
    }

    #[must_use]
    pub fn drop(&self, drop: DropId) -> Option<ExecutionFacts> {
        self.drops.get(drop).copied()
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::CallableGuarantees;

    use super::{AllocationFact, ExecutionFacts, SynchronousWaitFact};

    #[test]
    fn external_contract_conversion_preserves_both_independent_bounds() {
        assert_eq!(
            ExecutionFacts::admitted_by(CallableGuarantees::default()),
            ExecutionFacts::new(
                AllocationFact::MayAllocate,
                SynchronousWaitFact::Nonblocking,
            )
        );
        assert_eq!(
            ExecutionFacts::admitted_by(CallableGuarantees::no_allocation().admit_blocking()),
            ExecutionFacts::new(AllocationFact::NoAllocation, SynchronousWaitFact::MayBlock,)
        );
    }

    #[test]
    fn direct_fact_join_is_monotonic_and_independent_per_axis() {
        let mut facts = ExecutionFacts::default();
        assert!(facts.include(ExecutionFacts::allocation_request()));
        assert_eq!(
            facts,
            ExecutionFacts::new(
                AllocationFact::MayAllocate,
                SynchronousWaitFact::Nonblocking,
            )
        );
        assert!(facts.include(ExecutionFacts::new(
            AllocationFact::NoAllocation,
            SynchronousWaitFact::MayBlock,
        )));
        assert!(!facts.include(ExecutionFacts::allocation_request()));
        assert_eq!(
            facts,
            ExecutionFacts::new(AllocationFact::MayAllocate, SynchronousWaitFact::MayBlock,)
        );
    }
}
