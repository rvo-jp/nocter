//! Name-free semantic classification for source operator callables.

use crate::ast::{ComparisonOperatorKind, MethodReceiverMode, OperatorDecl};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum OperatorCallableKind {
    Equality,
    StrictOrder,
    ReadonlyIndex,
    ReadwriteIndex,
    ReadonlyExpansion,
    ReadwriteExpansion,
    OwnedExpansion,
}

impl OperatorCallableKind {
    pub(crate) const fn for_comparison(kind: ComparisonOperatorKind) -> Self {
        match kind {
            ComparisonOperatorKind::Equality => Self::Equality,
            ComparisonOperatorKind::StrictOrder => Self::StrictOrder,
        }
    }

    pub(crate) fn from_declaration(declaration: &OperatorDecl) -> Self {
        match declaration {
            OperatorDecl::Comparison(operator) => match operator.kind {
                ComparisonOperatorKind::Equality => Self::Equality,
                ComparisonOperatorKind::StrictOrder => Self::StrictOrder,
            },
            OperatorDecl::Index(operator) => match operator.callable().receiver.mode {
                MethodReceiverMode::ReadonlyBorrow => Self::ReadonlyIndex,
                MethodReceiverMode::ReadwriteBorrow => Self::ReadwriteIndex,
                MethodReceiverMode::Owned => unreachable!("validated index receiver"),
            },
            OperatorDecl::Expansion(operator) => match operator.callable().receiver.mode {
                MethodReceiverMode::ReadonlyBorrow => Self::ReadonlyExpansion,
                MethodReceiverMode::ReadwriteBorrow => Self::ReadwriteExpansion,
                MethodReceiverMode::Owned => Self::OwnedExpansion,
            },
        }
    }

    pub(crate) const fn lookup_name(self) -> &'static str {
        match self {
            Self::Equality => "__nocter$operator$equal",
            Self::StrictOrder => "__nocter$operator$less",
            Self::ReadonlyIndex => "__nocter$operator$index",
            Self::ReadwriteIndex => "__nocter$operator$index_readwrite",
            Self::ReadonlyExpansion => "__nocter$operator$expand_readonly",
            Self::ReadwriteExpansion => "__nocter$operator$expand_readwrite",
            Self::OwnedExpansion => "__nocter$operator$expand_owned",
        }
    }

    pub(crate) fn from_lookup_name(name: &str) -> Option<Self> {
        [
            Self::Equality,
            Self::StrictOrder,
            Self::ReadonlyIndex,
            Self::ReadwriteIndex,
            Self::ReadonlyExpansion,
            Self::ReadwriteExpansion,
            Self::OwnedExpansion,
        ]
        .into_iter()
        .find(|kind| kind.lookup_name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_lookup_names_round_trip_through_typed_kinds() {
        for kind in [
            OperatorCallableKind::Equality,
            OperatorCallableKind::StrictOrder,
            OperatorCallableKind::ReadonlyIndex,
            OperatorCallableKind::ReadwriteIndex,
            OperatorCallableKind::ReadonlyExpansion,
            OperatorCallableKind::ReadwriteExpansion,
            OperatorCallableKind::OwnedExpansion,
        ] {
            assert_eq!(
                OperatorCallableKind::from_lookup_name(kind.lookup_name()),
                Some(kind)
            );
        }
    }
}
