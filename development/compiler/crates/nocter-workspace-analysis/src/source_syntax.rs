use std::sync::Arc;

use nocter_computation::{
    ComputationError, ComputationKey, Database, Fingerprint, Query, QueryValue,
};
use nocter_source::SourceFile;
use nocter_syntax::{
    ParseGoal, ParsedSyntax, SourceSyntaxError, SourceSyntaxProvider, parse_reusable,
};

#[derive(Clone)]
struct SourceSyntaxKey {
    source: SourceFile,
    goal: ParseGoal,
    fingerprint: Fingerprint,
}

impl SourceSyntaxKey {
    fn new(source: &SourceFile, goal: ParseGoal) -> Self {
        let mut identity = Vec::with_capacity(1 + source.text().len());
        identity.extend_from_slice(goal.as_str().as_bytes());
        identity.push(0);
        identity.extend_from_slice(source.text().as_bytes());
        Self {
            source: source.clone(),
            goal,
            fingerprint: Fingerprint::from_bytes(&identity),
        }
    }
}

impl ComputationKey for SourceSyntaxKey {
    fn stable_bytes(&self) -> Box<[u8]> {
        self.fingerprint.digest().into()
    }
}

struct SourceSyntaxQuery;

struct SourceSyntaxProduct {
    syntax: Arc<ParsedSyntax>,
    fingerprint: Fingerprint,
}

impl QueryValue for SourceSyntaxProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for SourceSyntaxQuery {
    type Key = SourceSyntaxKey;
    type Value = SourceSyntaxProduct;

    fn execute(_: &Database, key: &SourceSyntaxKey) -> Result<Self::Value, ComputationError> {
        Ok(SourceSyntaxProduct {
            syntax: Arc::new(parse_reusable(&key.source, key.goal)),
            fingerprint: key.fingerprint,
        })
    }
}

pub(super) struct ComputedSourceSyntax<'database> {
    database: &'database Database,
}

impl<'database> ComputedSourceSyntax<'database> {
    pub(super) const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl SourceSyntaxProvider for ComputedSourceSyntax<'_> {
    fn parsed_syntax(
        &mut self,
        source: &SourceFile,
        goal: ParseGoal,
    ) -> Result<Arc<ParsedSyntax>, SourceSyntaxError> {
        let syntax = self
            .database
            .query::<SourceSyntaxQuery>(SourceSyntaxKey::new(source, goal))
            .map_err(SourceSyntaxError::new)?;
        Ok(Arc::clone(&syntax.syntax))
    }
}

#[cfg(test)]
pub(super) fn execution_count(database: &Database) -> u64 {
    database.execution_count::<SourceSyntaxQuery>()
}

#[cfg(test)]
pub(super) fn reuse_count(database: &Database) -> u64 {
    database.reuse_count::<SourceSyntaxQuery>()
}
