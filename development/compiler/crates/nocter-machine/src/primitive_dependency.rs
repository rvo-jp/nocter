use nocter_model::TypeId;

/// Physical element placement for one two-element asynchronous tuple result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineAsyncTuplePlan {
    first_output_offset: u64,
    second_output_offset: u64,
}

impl MachineAsyncTuplePlan {
    pub(crate) const fn new(first_output_offset: u64, second_output_offset: u64) -> Self {
        Self {
            first_output_offset,
            second_output_offset,
        }
    }

    #[must_use]
    pub const fn first_output_offset(self) -> u64 {
        self.first_output_offset
    }

    #[must_use]
    pub const fn second_output_offset(self) -> u64 {
        self.second_output_offset
    }
}

/// Specialized semantic work retained by a machine primitive target.
///
/// A no-op destruction dependency explicitly records that the subject is copyable. Nontrivial
/// destruction is already an ordinary direct machine call, so recursive plans cannot cross this
/// boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachinePrimitiveDependency {
    None,
    NoopDestruction { subject: TypeId },
    AsyncTuple(MachineAsyncTuplePlan),
}
