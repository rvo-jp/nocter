//! Data and input operations remain separate at the source-independent call boundary.
use crate::RuntimeDataId;
use nocter_model::{AccessLocalId, ValueInputOperation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeValueInput {
    pub local: AccessLocalId,
    pub data: RuntimeDataId,
    pub operation: ValueInputOperation,
}

/// Exact checked body channels, with no stored shape invented for an access operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeValueSignature {
    environment: Option<RuntimeValueInput>,
    explicit: Box<[RuntimeValueInput]>,
    result: RuntimeDataId,
}

impl RuntimeValueSignature {
    #[must_use]
    pub fn new(
        environment: Option<RuntimeValueInput>,
        explicit: impl Into<Box<[RuntimeValueInput]>>,
        result: RuntimeDataId,
    ) -> Self {
        Self {
            environment,
            explicit: explicit.into(),
            result,
        }
    }

    #[must_use]
    pub const fn environment(&self) -> Option<RuntimeValueInput> {
        self.environment
    }

    #[must_use]
    pub fn explicit(&self) -> &[RuntimeValueInput] {
        &self.explicit
    }

    #[must_use]
    pub const fn result(&self) -> RuntimeDataId {
        self.result
    }

    pub fn ordered(&self) -> impl Iterator<Item = RuntimeValueInput> + '_ {
        self.environment
            .into_iter()
            .chain(self.explicit.iter().copied())
    }
}
