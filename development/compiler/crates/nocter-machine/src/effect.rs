use crate::{
    MachineArithmeticCheck, MachineBinaryOperation, MachineOperationKind, MachineUnaryOperation,
};

/// The strongest removal guarantee Machine can prove from an operation's closed kind alone.
///
/// This is deliberately conservative. More precise path or type evidence belongs in an explicit
/// optimization proof, never in an optimistic fallback classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MachineOperationEffect {
    /// Produces only its result and cannot trap. It may be removed when that result is unused.
    Pure,
    /// Has no intended external effect, but evaluation can trap under source semantics.
    MayTrap,
    /// Changes owned state, crosses an effect boundary, or participates in resource lifetime.
    Observable,
}

impl MachineOperationKind {
    pub(crate) const fn effect(&self) -> MachineOperationEffect {
        match self {
            Self::Constant(_)
            | Self::BorrowWeakening { .. }
            | Self::Aggregate(_)
            | Self::PackLength
            | Self::Unary {
                operation: MachineUnaryOperation::LogicalNot,
                ..
            }
            | Self::Unary {
                check:
                    MachineArithmeticCheck::NotRequired
                    | MachineArithmeticCheck::InternalInvariant
                    | MachineArithmeticCheck::ProvenSafe,
                ..
            }
            | Self::Binary {
                operation: MachineBinaryOperation::Equal | MachineBinaryOperation::Less,
                ..
            }
            | Self::Binary {
                check:
                    MachineArithmeticCheck::NotRequired
                    | MachineArithmeticCheck::InternalInvariant
                    | MachineArithmeticCheck::ProvenSafe,
                ..
            } => MachineOperationEffect::Pure,
            Self::Unary {
                check: MachineArithmeticCheck::Required | MachineArithmeticCheck::ProvenTrap,
                ..
            }
            | Self::Load { .. }
            | Self::AddressOf { .. }
            | Self::Binary {
                check: MachineArithmeticCheck::Required | MachineArithmeticCheck::ProvenTrap,
                ..
            }
            | Self::NumericConversion { .. }
            | Self::Comparison(_)
            | Self::IndexBorrow(_) => MachineOperationEffect::MayTrap,
            Self::Store { .. }
            | Self::EraseCallable(_)
            | Self::InvokeDrop { .. }
            | Self::ReportError { .. }
            | Self::ReleaseError { .. }
            | Self::ReleaseComputation { .. }
            | Self::ReleaseErasedCallable { .. }
            | Self::ReleaseMappedStorage { .. }
            | Self::DriveComputation { .. }
            | Self::CreateRegion { .. }
            | Self::ReleaseRegion { .. }
            | Self::SetDropFlag { .. }
            | Self::Call(_)
            | Self::PackNext
            | Self::DestroyPack => MachineOperationEffect::Observable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MachineOperationEffect;
    use crate::identity::MachineId;
    use crate::{MachineAddressId, MachineOperationKind, MachineUnaryOperation, MachineValueId};

    #[test]
    fn trapping_unary_operation_is_not_classified_as_removable() {
        let operation = MachineOperationKind::Unary {
            operation: MachineUnaryOperation::Negate,
            operand: MachineValueId::new(0),
            check: crate::MachineArithmeticCheck::Required,
        };
        assert_eq!(operation.effect(), MachineOperationEffect::MayTrap);
    }

    #[test]
    fn address_evaluation_is_not_pure_without_an_exact_address_proof() {
        let source = MachineAddressId::new(0);
        assert_eq!(
            MachineOperationKind::Load { source }.effect(),
            MachineOperationEffect::MayTrap
        );
        assert_eq!(
            MachineOperationKind::AddressOf { source }.effect(),
            MachineOperationEffect::MayTrap
        );
    }
}
