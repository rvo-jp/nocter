use nocter_model::{Arena, CallableId, ClosureId, DropId};

/// Positive allocation fact inferred for one executable semantic root.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum AllocationEffect {
    /// No reachable operation in the checked root requests new storage.
    #[default]
    NoAllocation,
    /// At least one reachable operation may request new storage.
    MayAllocate,
}

impl AllocationEffect {
    #[must_use]
    pub const fn may_allocate(self) -> bool {
        matches!(self, Self::MayAllocate)
    }
}

/// Positive synchronous-wait fact inferred for one executable semantic root.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum BlockingEffect {
    /// No reachable operation synchronously waits for external progress.
    #[default]
    Nonblocking,
    /// At least one reachable operation may synchronously wait for external progress.
    MayBlock,
}

impl BlockingEffect {
    #[must_use]
    pub const fn may_block(self) -> bool {
        matches!(self, Self::MayBlock)
    }
}

/// Independently inferred effects for one executable semantic root.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct CallableEffects {
    allocation: AllocationEffect,
    blocking: BlockingEffect,
}

impl CallableEffects {
    #[must_use]
    pub const fn new(allocation: AllocationEffect, blocking: BlockingEffect) -> Self {
        Self {
            allocation,
            blocking,
        }
    }

    #[must_use]
    pub const fn allocation(self) -> AllocationEffect {
        self.allocation
    }

    #[must_use]
    pub const fn blocking(self) -> BlockingEffect {
        self.blocking
    }

    pub(crate) fn include(&mut self, other: Self) -> bool {
        let next = Self {
            allocation: if self.allocation.may_allocate() || other.allocation.may_allocate() {
                AllocationEffect::MayAllocate
            } else {
                AllocationEffect::NoAllocation
            },
            blocking: if self.blocking.may_block() || other.blocking.may_block() {
                BlockingEffect::MayBlock
            } else {
                BlockingEffect::Nonblocking
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

/// Whole-program least-fixed-point callable-effect facts.
#[derive(Clone, Debug)]
pub struct EffectTable {
    callables: Arena<CallableId, CallableEffects>,
    closures: Arena<ClosureId, CallableEffects>,
    drops: Arena<DropId, CallableEffects>,
}

impl EffectTable {
    pub(super) const fn new(
        callables: Arena<CallableId, CallableEffects>,
        closures: Arena<ClosureId, CallableEffects>,
        drops: Arena<DropId, CallableEffects>,
    ) -> Self {
        Self {
            callables,
            closures,
            drops,
        }
    }

    #[must_use]
    pub fn callable(&self, callable: CallableId) -> Option<CallableEffects> {
        self.callables.get(callable).copied()
    }

    #[must_use]
    pub fn closure(&self, closure: ClosureId) -> Option<CallableEffects> {
        self.closures.get(closure).copied()
    }

    #[must_use]
    pub fn drop(&self, drop: DropId) -> Option<CallableEffects> {
        self.drops.get(drop).copied()
    }
}
