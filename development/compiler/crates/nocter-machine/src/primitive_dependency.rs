use nocter_model::TypeId;

use crate::MachineDestructionPlan;

/// Specialized semantic work retained by a machine primitive target.
///
/// A destruction dependency is explicit even when the subject is copyable (`plan: None`). This
/// keeps target lowering from reconstructing semantic meaning from a primitive role or type
/// argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachinePrimitiveDependency {
    None,
    Destruction {
        subject: TypeId,
        plan: Option<Box<MachineDestructionPlan>>,
    },
}

impl MachinePrimitiveDependency {
    #[must_use]
    pub fn destruction(&self) -> Option<(TypeId, Option<&MachineDestructionPlan>)> {
        match self {
            Self::None => None,
            Self::Destruction { subject, plan } => Some((*subject, plan.as_deref())),
        }
    }
}
