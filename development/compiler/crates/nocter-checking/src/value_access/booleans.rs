//! Normal-result facts, never permission to omit expression evaluation or to read storage early.
use super::{Binary, ExpressionKind, Unary, checker::Checker};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Demand {
    Never,
    Conditional,
    Always,
}
/// The same selected operator controls RHS reachability and possible normal completion in typing.
pub(super) fn right_demand(operation: Binary, left: Option<bool>) -> Demand {
    match (operation, left) {
        (Binary::And, Some(false)) | (Binary::Or, Some(true)) => Demand::Never,
        (Binary::And | Binary::Or, None) => Demand::Conditional,
        _ => Demand::Always,
    }
}
impl Checker<'_, '_> {
    /// Derive once while children are available. Flow and capture inference consume this fact;
    /// they cannot derive a different predicate from a place's current contents or a callee body.
    pub(super) fn boolean_result(&self, kind: &ExpressionKind) -> Option<bool> {
        match *kind {
            ExpressionKind::Boolean(value) => Some(value),
            ExpressionKind::Boundary { source, .. } => self.normal_boolean(source),
            ExpressionKind::Unary {
                operation: Unary::Not,
                operand,
            } => self.normal_boolean(operand).map(|v| !v),
            ExpressionKind::Binary {
                operation: Binary::And,
                left,
                right,
            } => match (self.normal_boolean(left), self.normal_boolean(right)) {
                (Some(false), _) | (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ if !self.ty(right).may_complete() => Some(false),
                _ => None,
            },
            ExpressionKind::Binary {
                operation: Binary::Or,
                left,
                right,
            } => match (self.normal_boolean(left), self.normal_boolean(right)) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ if !self.ty(right).may_complete() => Some(true),
                _ => None,
            },
            _ => None,
        }
    }
}
