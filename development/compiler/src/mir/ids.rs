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
