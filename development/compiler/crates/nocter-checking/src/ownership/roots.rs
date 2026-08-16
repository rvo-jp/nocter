use nocter_declarations::{BodyOwner, DeclarationGraph};

use crate::{BodySource, PlaceRoot};

/// Selects the owned parameter roots initialized when one body begins.
pub(crate) fn initialized_body_roots(
    graph: &DeclarationGraph,
    source: BodySource<'_>,
) -> Option<Vec<PlaceRoot>> {
    match source.owner() {
        BodyOwner::Callable(callable) => {
            let declaration = graph.declarations().callables().get(callable)?;
            Some(
                declaration
                    .receiver()
                    .into_iter()
                    .chain(declaration.parameters().iter().copied())
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
