//! Original returned facts are selected through declaration-owned callable result descriptors.
use super::{INVALID, State, Store, facts::Facts};
use crate::value_access::{
    Query,
    flow::{Aliases, IntegrityError},
};
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema};
use nocter_model::{AccessExpressionId, ValueCallableResult, ValueCode, ValuePath};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Returns {
    paths: BTreeMap<usize, ValuePath<()>>,
    returned: BTreeMap<usize, BTreeSet<(FunctionWitness, AccessExpressionId)>>,
}
impl Returns {
    pub(super) fn new(query: &Query) -> Result<Self, IntegrityError> {
        let paths = paths(query)?;
        let mut returned = BTreeMap::new();
        if paths.get(&0).is_some_and(ValuePath::is_root) {
            returned.insert(0, BTreeSet::new());
        }
        Ok(Self { paths, returned })
    }
    pub(super) fn stage(
        &mut self,
        query: &Query,
        id: AccessExpressionId,
        state: &State,
        facts: &mut Store,
        aliases: &Aliases,
    ) -> Result<(), IntegrityError> {
        let value = if query
            .body
            .expressions
            .get(id)
            .ok_or(INVALID)?
            .ty
            .access()
            .is_some()
        {
            super::shape(query, id, state, false, facts, aliases)?
        } else {
            Facts::default()
        };
        for (&ordinal, path) in &self.paths {
            self.returned.entry(ordinal).or_default().extend(
                facts
                    .producers(facts.selected(value, path.fields()))
                    .into_iter()
                    .map(|witness| (witness, id)),
            );
        }
        Ok(())
    }
    pub(super) fn finish(self) -> BTreeMap<usize, Vec<(FunctionWitness, AccessExpressionId)>> {
        self.returned
            .into_iter()
            .map(|(ordinal, values)| (ordinal, values.into_iter().collect()))
            .collect()
    }
}

fn paths(query: &Query) -> Result<BTreeMap<usize, ValuePath<()>>, IntegrityError> {
    let mut paths = BTreeMap::new();
    if let ValueCode::Declaration(owner) = query.code() {
        let code = query.program().codes().get(owner).ok_or(INVALID)?;
        for ordinal in 0..code.result_contracts().len() {
            let path = query
                .program()
                .callable_result_path(ValueCallableResult { owner, ordinal })
                .ok_or(INVALID)?;
            if query
                .program()
                .project_data_by(code.result(), &path, |(), length| length > 0)
                .is_some()
            {
                paths.insert(ordinal, path);
            }
        }
    } else if matches!(
        query.program().data_schema(query.result_data()),
        Some(PayloadSchema::Function(_))
    ) {
        paths.insert(0, ValuePath::default());
    }
    Ok(paths)
}
