use nocter_model::TypeStore;

use crate::{RuntimeAbiIdentity, RuntimeTypeRepresentationTable};

/// The complete source-independent environment required after MIR lowering.
///
/// Target construction freezes these facts once. MIR retains this value as its backend boundary;
/// machine lowering cannot recover semantic or source authority through it.
#[derive(Clone, Debug)]
pub struct RuntimeEnvironment {
    types: TypeStore,
    type_representations: RuntimeTypeRepresentationTable,
    abi: RuntimeAbiIdentity,
}

impl RuntimeEnvironment {
    #[must_use]
    pub fn new(
        types: TypeStore,
        type_representations: RuntimeTypeRepresentationTable,
        abi: RuntimeAbiIdentity,
    ) -> Self {
        Self {
            types,
            type_representations,
            abi,
        }
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        &self.types
    }

    #[must_use]
    pub const fn type_representations(&self) -> &RuntimeTypeRepresentationTable {
        &self.type_representations
    }

    #[must_use]
    pub const fn abi(&self) -> RuntimeAbiIdentity {
        self.abi
    }
}
