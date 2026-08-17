use nocter_model::{CallableCapability, CaptureId, ClosureId, TypeId};

/// One concrete field in a monomorphized closure environment.
///
/// The type is the runtime representation stored by the closure. Borrow captures therefore retain
/// their concrete borrow type here rather than the referent type exposed inside the closure body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableClosureCapture {
    binding: CaptureId,
    ty: TypeId,
}

impl ExecutableClosureCapture {
    pub(super) const fn new(binding: CaptureId, ty: TypeId) -> Self {
        Self { binding, ty }
    }

    #[must_use]
    pub const fn binding(self) -> CaptureId {
        self.binding
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// The complete concrete runtime layout owned by one executable closure item.
///
/// Generic substitution ends before this value is created. MIR and backends consume this frozen
/// layout and never reopen the checked closure definition to recover capture representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableClosureLayout {
    closure: ClosureId,
    ty: TypeId,
    capability: CallableCapability,
    captures: Box<[ExecutableClosureCapture]>,
}

impl ExecutableClosureLayout {
    pub(super) fn new(
        closure: ClosureId,
        ty: TypeId,
        capability: CallableCapability,
        captures: impl Into<Box<[ExecutableClosureCapture]>>,
    ) -> Self {
        Self {
            closure,
            ty,
            capability,
            captures: captures.into(),
        }
    }

    #[must_use]
    pub const fn closure(&self) -> ClosureId {
        self.closure
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn capability(&self) -> CallableCapability {
        self.capability
    }

    #[must_use]
    pub const fn captures(&self) -> &[ExecutableClosureCapture] {
        &self.captures
    }

    #[must_use]
    pub fn capture(&self, binding: CaptureId) -> Option<ExecutableClosureCapture> {
        self.captures
            .iter()
            .copied()
            .find(|capture| capture.binding == binding)
    }
}
