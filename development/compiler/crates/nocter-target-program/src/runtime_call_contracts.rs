use std::collections::BTreeSet;
use std::fmt;

use nocter_declarations::{CallableKind, DeclarationGraph};
use nocter_model::CallableId;

use crate::ToolchainSnapshot;

/// A primitive declaration that is not owned by either closed runtime-call catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnregisteredRuntimeCall {
    callable: CallableId,
}

impl UnregisteredRuntimeCall {
    #[must_use]
    pub const fn callable(self) -> CallableId {
        self.callable
    }
}

impl fmt::Display for UnregisteredRuntimeCall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("standard package declares an unregistered primitive")
    }
}

impl std::error::Error for UnregisteredRuntimeCall {}

/// Proves that primitive syntax cannot introduce a third, unvalidated runtime-call authority.
pub(crate) fn validate_runtime_call_coverage(
    graph: &DeclarationGraph,
    snapshot: &ToolchainSnapshot,
) -> Result<(), UnregisteredRuntimeCall> {
    let registered = snapshot
        .primitives()
        .bindings()
        .iter()
        .map(|binding| binding.callable())
        .chain(
            snapshot
                .target_services()
                .bindings()
                .iter()
                .map(nocter_runtime_contract::TargetServiceBinding::callable),
        )
        .collect::<BTreeSet<_>>();
    graph
        .declarations()
        .callables()
        .iter()
        .find(|(callable, declaration)| {
            declaration.kind() == CallableKind::Primitive && !registered.contains(callable)
        })
        .map_or(Ok(()), |(callable, _)| {
            Err(UnregisteredRuntimeCall { callable })
        })
}
