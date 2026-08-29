use nocter_computation::{
    ComputationError, ComputationKey, Database, Fingerprint, Query, QueryValue,
};
use nocter_discovery::{SourceSyntaxError, SourceSyntaxProvider};
use nocter_source::SourceFile;
use nocter_syntax::{ParseGoal, ParsedSyntax, SyntaxTree, parse_reusable};

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
    syntax: ParsedSyntax,
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
            syntax: parse_reusable(&key.source, key.goal),
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
    fn syntax(
        &mut self,
        source: &SourceFile,
        goal: ParseGoal,
    ) -> Result<SyntaxTree, SourceSyntaxError> {
        let syntax = self
            .database
            .query::<SourceSyntaxQuery>(SourceSyntaxKey::new(source, goal))
            .map_err(SourceSyntaxError::new)?;
        syntax
            .syntax
            .bind(source)
            .ok_or_else(|| SourceSyntaxError::new(SourceBindingMismatch))
    }
}

#[derive(Debug)]
struct SourceBindingMismatch;

impl std::fmt::Display for SourceBindingMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("cached syntax text does not match the current source")
    }
}

impl std::error::Error for SourceBindingMismatch {}

#[cfg(test)]
pub(super) fn execution_count(database: &Database) -> u64 {
    database.execution_count::<SourceSyntaxQuery>()
}

#[cfg(test)]
pub(super) fn reuse_count(database: &Database) -> u64 {
    database.reuse_count::<SourceSyntaxQuery>()
}
