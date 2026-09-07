use nocter_runtime_contract::{PrimitiveBinding, TargetServiceBinding};

/// Source-neutral semantic bindings for the two closed runtime-call catalogs.
#[derive(Debug)]
pub(crate) struct RuntimeCallBindings {
    primitives: Box<[PrimitiveBinding]>,
    target_services: Box<[TargetServiceBinding]>,
}

impl RuntimeCallBindings {
    #[must_use]
    pub(crate) const fn new(
        primitives: Box<[PrimitiveBinding]>,
        target_services: Box<[TargetServiceBinding]>,
    ) -> Self {
        Self {
            primitives,
            target_services,
        }
    }

    #[must_use]
    pub(crate) const fn primitives(&self) -> &[PrimitiveBinding] {
        &self.primitives
    }

    #[must_use]
    pub(crate) const fn target_services(&self) -> &[TargetServiceBinding] {
        &self.target_services
    }
}
