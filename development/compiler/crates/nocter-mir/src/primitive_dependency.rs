use nocter_model::TypeId;

use crate::MirDestructionPlan;

/// Specialized semantic work carried by a primitive call after executable closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirPrimitiveDependency {
    None,
    Destruction {
        subject: TypeId,
        plan: Option<Box<MirDestructionPlan>>,
    },
}

impl MirPrimitiveDependency {
    #[must_use]
    pub fn destruction(&self) -> Option<(TypeId, Option<&MirDestructionPlan>)> {
        match self {
            Self::None => None,
            Self::Destruction { subject, plan } => Some((*subject, plan.as_deref())),
        }
    }
}
