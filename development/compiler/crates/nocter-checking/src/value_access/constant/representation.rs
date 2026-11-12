//! A projection can name an unavailable payload without producing a value in this domain.
use super::{INVALID, flow};
use crate::value_access::{ExpressionKind, evaluation};
use nocter_model::AccessExpressionId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct Projections {
    uninhabited: BTreeMap<AccessExpressionId, bool>,
}

impl Projections {
    /// Called only for a result with no evaluator representation. Selecting it from an admitted
    /// carrier cannot produce a value: validation admits only its representable cases. This grants
    /// neither an error value nor an operation on one. Transparent boundaries preserve that fact.
    pub(super) fn uninhabited(
        &mut self,
        query: &flow::Query,
        mut id: AccessExpressionId,
        domain: &mut evaluation::Types,
    ) -> Result<bool, flow::IntegrityError> {
        let expressions = query.typed().body().expressions();
        let mut visited = BTreeSet::new();
        let result = loop {
            if let Some(&result) = self.uninhabited.get(&id) {
                break result;
            }
            if !visited.insert(id) {
                return Err(INVALID);
            }
            let expression = expressions.get(id).ok_or(INVALID)?;
            let Some(value) = expression.ty.normal() else {
                break false;
            };
            if domain.supports(query.typed().program(), value.data())? {
                break false;
            }
            match expression.kind {
                ExpressionKind::Select { source, .. } => {
                    let Some(value) = expressions.get(source).ok_or(INVALID)?.ty.normal() else {
                        break false;
                    };
                    if domain.supports(query.typed().program(), value.data())? {
                        break true;
                    }
                    id = source;
                }
                ExpressionKind::Boundary { source, .. } => id = source,
                _ => break false,
            }
        };
        self.uninhabited
            .extend(visited.into_iter().map(|id| (id, result)));
        Ok(result)
    }
}
