//! A checked carrier's domain obligation also covers its original payload projections.
use super::{INVALID, flow};
use crate::value_access::{ExpressionKind, evaluation};
use nocter_model::AccessExpressionId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct Projections {
    covered: BTreeMap<AccessExpressionId, bool>,
}

impl Projections {
    /// The carrier itself retains its ordinary bound domain condition. If it is admitted, its
    /// selected payload is representable or belongs to an inactive unrepresentable alternative.
    /// A conditional carrier contract does not fabricate an actual payload or an error value.
    pub(super) fn covered(
        &mut self,
        query: &flow::Query,
        mut id: AccessExpressionId,
        domain: &mut evaluation::Types,
    ) -> Result<bool, flow::IntegrityError> {
        let expressions = query.typed().body().expressions();
        let mut visited = BTreeSet::new();
        let result = loop {
            if let Some(&result) = self.covered.get(&id) {
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
                    if domain.supports_contract(query.typed().program(), value.data())? {
                        break true;
                    }
                    id = source;
                }
                ExpressionKind::Boundary { source, .. } => id = source,
                _ => break false,
            }
        };
        self.covered
            .extend(visited.into_iter().map(|id| (id, result)));
        Ok(result)
    }
}
