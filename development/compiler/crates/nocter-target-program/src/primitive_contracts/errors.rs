use std::fmt;

use nocter_model::CallableId;

use crate::PrimitiveRole;

/// The exact part of a registered primitive declaration that failed validation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrimitiveContractRule {
    Authority,
    Module,
    Name,
    CallableKind,
    Visibility,
    GenericShape,
    ParameterShape,
    ResultType,
    Provenance,
    TargetGate,
    Body,
    Requirements,
    SupportingType,
}

/// One compiler-owned role attached to a declaration that does not satisfy its closed contract.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PrimitiveContractError {
    role: PrimitiveRole,
    callable: CallableId,
    rule: PrimitiveContractRule,
}

impl PrimitiveContractError {
    pub(super) const fn new(
        role: PrimitiveRole,
        callable: CallableId,
        violated_rule: PrimitiveContractRule,
    ) -> Self {
        Self {
            role,
            callable,
            rule: violated_rule,
        }
    }

    #[must_use]
    pub const fn role(self) -> PrimitiveRole {
        self.role
    }

    #[must_use]
    pub const fn callable(self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn rule(self) -> PrimitiveContractRule {
        self.rule
    }
}

impl fmt::Debug for PrimitiveContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrimitiveContractError")
            .field("role", &self.role)
            .field("callable", &self.callable)
            .field("rule", &self.rule)
            .finish()
    }
}

impl fmt::Display for PrimitiveContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "primitive {:?} violates its {:?} contract",
            self.role, self.rule
        )
    }
}

impl std::error::Error for PrimitiveContractError {}

/// Failure to prove that the checked standard package is exactly the compiler registry.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum PrimitiveRegistryValidationError {
    Contract(PrimitiveContractError),
    UnregisteredPrimitive(CallableId),
}

impl fmt::Debug for PrimitiveRegistryValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => formatter.debug_tuple("Contract").field(error).finish(),
            Self::UnregisteredPrimitive(callable) => formatter
                .debug_tuple("UnregisteredPrimitive")
                .field(callable)
                .finish(),
        }
    }
}

impl fmt::Display for PrimitiveRegistryValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => error.fmt(formatter),
            Self::UnregisteredPrimitive(_) => {
                formatter.write_str("standard package declares an unregistered primitive")
            }
        }
    }
}

impl std::error::Error for PrimitiveRegistryValidationError {}
