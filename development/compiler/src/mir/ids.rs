//! Opaque identities local to one MIR body.

macro_rules! mir_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub(crate) struct $name(u32);

        impl $name {
            pub(crate) const fn from_index(index: usize) -> Self {
                Self(index as u32)
            }

            pub(crate) const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

mir_id!(BasicBlockId);
mir_id!(LocalId);
mir_id!(LoanId);
mir_id!(DropPlanId);
#[allow(
    dead_code,
    reason = "aggregate route construction follows the projected-place validation checkpoint"
)]
mod projection_path_id {
    mir_id!(ProjectionPathId);
}
pub(crate) use projection_path_id::ProjectionPathId;
mir_id!(ScopeId);
