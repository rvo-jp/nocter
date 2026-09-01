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

/// Whole-program least-fixed-point allocation facts.
#[derive(Clone, Debug)]
pub struct EffectTable {
    callables: Arena<CallableId, AllocationEffect>,
    closures: Arena<ClosureId, AllocationEffect>,
    drops: Arena<DropId, AllocationEffect>,
}

impl EffectTable {
    pub(super) const fn new(
        callables: Arena<CallableId, AllocationEffect>,
        closures: Arena<ClosureId, AllocationEffect>,
        drops: Arena<DropId, AllocationEffect>,
    ) -> Self {
        Self {
            callables,
            closures,
            drops,
        }
    }

    #[must_use]
    pub fn callable(&self, callable: CallableId) -> Option<AllocationEffect> {
        self.callables.get(callable).copied()
    }

    #[must_use]
    pub fn closure(&self, closure: ClosureId) -> Option<AllocationEffect> {
        self.closures.get(closure).copied()
    }

    #[must_use]
    pub fn drop(&self, drop: DropId) -> Option<AllocationEffect> {
        self.drops.get(drop).copied()
    }
}
