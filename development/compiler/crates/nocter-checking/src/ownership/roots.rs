use nocter_declarations::{BodyOwner, DeclarationGraph, ParameterRole};
use nocter_model::CallableCapability;

use crate::{BodySource, PlaceRoot};

/// Selects the owned parameter roots initialized when one body begins.
pub(crate) fn initialized_body_roots(
    graph: &DeclarationGraph,
    source: BodySource<'_>,
) -> Option<Vec<PlaceRoot>> {
    match source.owner() {
        BodyOwner::Callable(callable) => {
            let declaration = graph.declarations().callables().get(callable)?;
            let parameters = graph.declarations().parameters();
            Some(
                declaration
                    .receiver()
                    .into_iter()
                    .chain(declaration.parameters().iter().copied())
                    .filter(|parameter| {
                        !parameters.get(*parameter).is_some_and(|parameter| {
                            matches!(parameter.role(), ParameterRole::ArgumentPack { .. })
                        })
                    })
                    .map(PlaceRoot::Parameter)
                    .collect(),
            )
        }
        BodyOwner::Drop(drop) => graph
            .declarations()
            .drops()
            .get(drop)
            .map(|declaration| vec![PlaceRoot::Parameter(declaration.receiver())]),
        BodyOwner::Test(_) => Some(Vec::new()),
    }
}

/// Selects parameter roots whose values are owned by one body invocation.
///
/// Borrowed receivers are initialized storage for ownership-flow validation, but the callee does
/// not own the referenced value and must never schedule its destruction. Ordinary parameters and
/// owned receivers are transferred into the invocation and remain the callee's responsibility.
pub(crate) fn owned_body_roots(
    graph: &DeclarationGraph,
    source: BodySource<'_>,
) -> Option<Vec<PlaceRoot>> {
    match source.owner() {
        BodyOwner::Callable(callable) => {
            let declarations = graph.declarations();
            let declaration = declarations.callables().get(callable)?;
            let mut roots = declaration
                .parameters()
                .iter()
                .copied()
                .filter(|parameter| {
                    !declarations
                        .parameters()
                        .get(*parameter)
                        .is_some_and(|parameter| {
                            matches!(parameter.role(), ParameterRole::ArgumentPack { .. })
                        })
                })
                .map(PlaceRoot::Parameter)
                .collect::<Vec<_>>();
            if let Some(receiver) = declaration.receiver() {
                let parameter = declarations.parameters().get(receiver)?;
                if parameter.role() == ParameterRole::Receiver(CallableCapability::Owned) {
                    roots.insert(0, PlaceRoot::Parameter(receiver));
                }
            }
            Some(roots)
        }
        BodyOwner::Drop(_) | BodyOwner::Test(_) => Some(Vec::new()),
    }
}
