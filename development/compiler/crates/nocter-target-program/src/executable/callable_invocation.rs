use nocter_checking::ConcreteDestructionPlan;
use nocter_model::{CallableContract, ExecutableItemId, TypeId};

/// One statically specialized invocation of a value accepted through a callable bound.
///
/// Callable contracts are not erased runtime types. Executable construction resolves the concrete
/// closure body and freezes any destruction that remains in the caller when an owned contract
/// invokes a closure body through readonly or readwrite access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableCallableInvocation {
    subject: TypeId,
    contract: CallableContract,
    body: ExecutableItemId,
    post_call_destruction: Option<ConcreteDestructionPlan>,
}

impl ExecutableCallableInvocation {
    pub(super) fn new(
        subject: TypeId,
        contract: CallableContract,
        body: ExecutableItemId,
        post_call_destruction: Option<ConcreteDestructionPlan>,
    ) -> Self {
        Self {
            subject,
            contract,
            body,
            post_call_destruction,
        }
    }

    #[must_use]
    pub const fn subject(&self) -> TypeId {
        self.subject
    }

    #[must_use]
    pub const fn contract(&self) -> &CallableContract {
        &self.contract
    }

    #[must_use]
    pub const fn body(&self) -> ExecutableItemId {
        self.body
    }

    #[must_use]
    pub const fn post_call_destruction(&self) -> Option<&ConcreteDestructionPlan> {
        self.post_call_destruction.as_ref()
    }
}
