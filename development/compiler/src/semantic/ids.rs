//! Opaque identities scoped to one immutable compile-unit semantic database.

macro_rules! semantic_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub(crate) struct $name(u32);

        impl $name {
            pub(super) const fn from_index(index: usize) -> Self {
                Self(index as u32)
            }

            pub(super) const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

semantic_id!(DefId);

#[cfg(test)]
impl DefId {
    pub(crate) const fn raw(self) -> u32 {
        self.0
    }

    pub(crate) const fn for_test(raw: u32) -> Self {
        Self(raw)
    }
}
