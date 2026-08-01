use crate::ast::{ArrayLiteralExpr, Expr};
use crate::ir::{Instruction, UsizeLocation, UsizeValue};

pub(in crate::ir::lower) fn array_literal_requires_runtime_progress(
    literal: &ArrayLiteralExpr,
) -> bool {
    literal.elements.iter().any(expression_propagates_failure)
}

fn expression_propagates_failure(expression: &Expr) -> bool {
    match expression {
        Expr::Propagate(_) => true,
        Expr::Group(group) => expression_propagates_failure(&group.expression),
        _ => false,
    }
}

/// Runtime progress for left-to-right aggregate initialization.
///
/// The counter is advanced only after an element has been fully written. A
/// failure raised while lowering the next element can therefore drop exactly
/// the completed prefix without touching uninitialized storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ir::lower) struct ArrayInitializationProgress {
    initialized: UsizeLocation,
}

impl ArrayInitializationProgress {
    pub(in crate::ir::lower) fn new(initialized: UsizeLocation) -> Self {
        Self { initialized }
    }

    pub(in crate::ir::lower) fn location(self) -> UsizeLocation {
        self.initialized
    }

    pub(in crate::ir::lower) fn initialize(self) -> Instruction {
        Instruction::SetUsize {
            destination: self.initialized,
            value: UsizeValue::Const(0),
        }
    }

    pub(in crate::ir::lower) fn complete_element(self, initialized_count: u64) -> Instruction {
        Instruction::SetUsize {
            destination: self.initialized,
            value: UsizeValue::Const(initialized_count),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_starts_empty_and_advances_after_a_complete_element() {
        let progress = ArrayInitializationProgress::new(UsizeLocation::Local(4));

        assert_eq!(
            progress.initialize(),
            Instruction::SetUsize {
                destination: UsizeLocation::Local(4),
                value: UsizeValue::Const(0),
            }
        );
        assert_eq!(
            progress.complete_element(2),
            Instruction::SetUsize {
                destination: UsizeLocation::Local(4),
                value: UsizeValue::Const(2),
            }
        );
    }
}
