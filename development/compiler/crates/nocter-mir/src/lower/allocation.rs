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
            AllocationSelection::CurrentRegion => Ok(MirCallAllocation::Inherit),
            AllocationSelection::Explicit(node) => {
                self.lower_place_node(node).map(MirCallAllocation::Explicit)
            }
        }
    }
}
