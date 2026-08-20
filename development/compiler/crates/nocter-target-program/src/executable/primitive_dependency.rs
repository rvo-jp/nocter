use nocter_checking::ConcreteDestructionPlan;
use nocter_model::TypeId;

/// Concrete semantic work required by a primitive beyond its ordinary runtime signature.
///
/// `None` means the target can select the primitive solely from its closed role and ABI. A
/// destruction dependency distinguishes a copyable subject (`plan: None`) from a primitive that
/// has no destruction semantics at all, so later lowering never has to infer that distinction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutablePrimitiveDependency {
    None,
    Destruction {
        subject: TypeId,
        plan: Option<Box<ConcreteDestructionPlan>>,
    },
}

impl ExecutablePrimitiveDependency {
    #[must_use]
    pub fn destruction(&self) -> Option<(TypeId, Option<&ConcreteDestructionPlan>)> {
        match self {
            Self::None => None,
            Self::Destruction { subject, plan } => Some((*subject, plan.as_deref())),
        }
    }
}
