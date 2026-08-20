use nocter_checking::AllocationSelection;

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::MirCallAllocation;

impl FunctionLowerer<'_> {
    /// Lowers the checked allocation choice without reading or moving the selected place.
    ///
    /// The call boundary temporarily exposes this place as the ambient allocator. Keeping it as a
    /// place preserves allocator identity and lets MIR validation enforce the compiler-selected
    /// allocator/context nominal roles.
    pub(super) fn lower_call_allocation(
        &mut self,
        allocation: AllocationSelection,
    ) -> Result<MirCallAllocation, MirLoweringError> {
        match allocation {
            AllocationSelection::CurrentRegion => Ok(self.current_call_allocation()),
            AllocationSelection::Explicit(node) => {
                self.lower_place_node(node).map(MirCallAllocation::Explicit)
            }
        }
    }

    /// Freezes the lexical current context at the call site. Outside a region the function-entry
    /// context remains authoritative; inside one, the exact non-movable region local is retained.
    pub(super) fn current_call_allocation(&self) -> MirCallAllocation {
        self.regions
            .last()
            .copied()
            .map_or(MirCallAllocation::Inherit, MirCallAllocation::Region)
    }
}
