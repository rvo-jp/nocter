//! Allocation and initialization of one compiler-owned two-child composition frame.

use crate::{
    Arm64AddSubtract, Arm64AsyncPairTargets, Arm64Code, Arm64CodeBuilder, Arm64DataSize,
    Arm64NocterAbi,
};

pub(super) const FIRST_STACK_OFFSET: u64 = 0;
pub(super) const SECOND_STACK_OFFSET: u64 = 8;
pub(super) const FIRST_OUTPUT_STACK_OFFSET: u64 = 16;
pub(super) const SECOND_OUTPUT_STACK_OFFSET: u64 = 24;
pub(super) const ALLOCATION_CONTEXT_STACK_OFFSET: u64 = 32;
const STACK_SIZE: u64 = 48;

/// Constructs one compiler-owned computation that takes ownership of two child computations.
pub(crate) fn materialize(
    targets: Arm64AsyncPairTargets,
) -> Result<Arm64Code, crate::Arm64CodeError> {
    let mut code = Arm64CodeBuilder::new();
    crate::async_pair_code::validate_nonzero(argument(0), &mut code);
    crate::async_pair_code::validate_nonzero(argument(1), &mut code);
    crate::frame_access::adjust_stack(&mut code, STACK_SIZE, Arm64AddSubtract::Subtract);
    store_stack(FIRST_STACK_OFFSET, argument(0), &mut code);
    store_stack(SECOND_STACK_OFFSET, argument(1), &mut code);
    store_stack(FIRST_OUTPUT_STACK_OFFSET, argument(2), &mut code);
    store_stack(SECOND_OUTPUT_STACK_OFFSET, argument(3), &mut code);
    store_stack(
        ALLOCATION_CONTEXT_STACK_OFFSET,
        Arm64NocterAbi::allocation_context_register(),
        &mut code,
    );

    crate::frame_access::load_immediate(
        &mut code,
        argument(1),
        crate::async_pair_code::PAIR_FRAME_SIZE,
        Arm64DataSize::Bits64,
    );
    crate::darwin_memory_code::emit_map(&mut code)?;
    crate::address_code::move_register(&mut code, argument(0), argument(6));
    crate::async_pair_code::initialize_pair_frame(argument(6), targets, &mut code)?;
    crate::address_code::move_register(&mut code, argument(6), argument(0));
    crate::frame_access::adjust_stack(&mut code, STACK_SIZE, Arm64AddSubtract::Add);
    crate::async_pair_code::return_to_caller(&mut code);
    code.finish()
}

fn store_stack(offset: u64, source: crate::Arm64Register, code: &mut Arm64CodeBuilder) {
    crate::async_pair_code::store_stack(offset, source, code);
}

const fn argument(index: u8) -> crate::Arm64Register {
    crate::async_pair_code::argument(index)
}
