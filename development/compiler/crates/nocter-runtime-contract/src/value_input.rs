//! Data and input operations remain separate at the source-independent call boundary.
use nocter_model::{AccessLocalId, TypeId, ValueInputOperation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeValueInput {
    pub local: AccessLocalId,
    pub data: TypeId,
    pub operation: ValueInputOperation,
}

/// Exact checked body channels, with no stored shape invented for an access operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeValueSignature {
    environment: Option<RuntimeValueInput>,
    explicit: Box<[RuntimeValueInput]>,
    result: TypeId,
}

impl RuntimeValueSignature {
    #[must_use]
    pub fn new(
        environment: Option<RuntimeValueInput>,
        explicit: impl Into<Box<[RuntimeValueInput]>>,
        result: TypeId,
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
    pub const fn result(&self) -> TypeId {
        self.result
    }

    pub fn ordered(&self) -> impl Iterator<Item = RuntimeValueInput> + '_ {
        self.environment
            .into_iter()
            .chain(self.explicit.iter().copied())
    }
}
