//! Exact owned-environment schemas, separate from structural callable constraints.
use super::{FunctionConstraint, Program};
use nocter_model::{TypeId, ValueClosure};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Closure {
    pub(super) environment: TypeId,
    pub(super) contract: FunctionConstraint,
}
impl Closure {
    #[must_use]
    pub const fn environment(&self) -> TypeId {
        self.environment
    }
    #[must_use]
    pub const fn contract(&self) -> &FunctionConstraint {
        &self.contract
    }
}
impl Program {
    #[must_use]
    pub fn closure(&self, id: ValueClosure) -> Option<&Closure> {
        self.closures.get(&id)
    }
}
