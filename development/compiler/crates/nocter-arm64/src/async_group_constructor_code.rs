//! Allocation and initialization of one borrowed dynamic child-set readiness frame.

use crate::{
    Arm64AddSubtract, Arm64AsyncGroupTargets, Arm64Code, Arm64CodeBuilder, Arm64DataSize,
    Arm64NocterAbi,
};

pub(super) const CHILD_POINTER_STACK_OFFSET: u64 = 0;
pub(super) const CHILD_COUNT_STACK_OFFSET: u64 = 8;
pub(super) const AVAILABLE_OUTPUT_STACK_OFFSET: u64 = 16;
pub(super) const INDEX_OUTPUT_STACK_OFFSET: u64 = 24;
pub(super) const ALLOCATION_CONTEXT_STACK_OFFSET: u64 = 32;
const STACK_SIZE: u64 = 48;

/// Constructs a future that borrows, but never consumes, the caller-owned child-handle view.
pub(crate) fn materialize(
    targets: Arm64AsyncGroupTargets,
) -> Result<Arm64Code, crate::Arm64CodeError> {
    let mut code = Arm64CodeBuilder::new();
    crate::frame_access::adjust_stack(&mut code, STACK_SIZE, Arm64AddSubtract::Subtract);
    for (offset, register) in [
        (CHILD_POINTER_STACK_OFFSET, argument(0)),
        (CHILD_COUNT_STACK_OFFSET, argument(1)),
        (AVAILABLE_OUTPUT_STACK_OFFSET, argument(2)),
        (INDEX_OUTPUT_STACK_OFFSET, argument(3)),
        (
            ALLOCATION_CONTEXT_STACK_OFFSET,
            Arm64NocterAbi::allocation_context_register(),
        ),
    ] {
        crate::async_composition_code::store_stack(offset, register, &mut code);
    }
    crate::frame_access::load_immediate(
        &mut code,
        argument(1),
        crate::async_group_code::GROUP_FRAME_SIZE,
        Arm64DataSize::Bits64,
    );
    crate::darwin_memory_code::emit_map(&mut code)?;
    crate::address_code::move_register(&mut code, argument(0), argument(6));
    crate::async_group_code::initialize_group_frame(argument(6), targets, &mut code)?;
    crate::address_code::move_register(&mut code, argument(6), argument(0));
    crate::frame_access::adjust_stack(&mut code, STACK_SIZE, Arm64AddSubtract::Add);
    crate::async_composition_code::return_to_caller(&mut code);
    code.finish()
}

const fn argument(index: u8) -> crate::Arm64Register {
    crate::async_composition_code::argument(index)
}
