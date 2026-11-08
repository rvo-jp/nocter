//! Shared source-only construction for private value-binding integration tests.
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{BoundSyntax, ParseGoal, SyntaxTree, parse};

/// Own exact source and production private-goal syntax, without constructing semantic evidence.
pub struct ValueSourceFixture {
    sources: SourceMap,
    tree: SyntaxTree,
}
impl ValueSourceFixture {
    /// # Panics
    /// Panics when the supplied string cannot form the immutable source input.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut sources = SourceMap::new();
        let id = sources
            .add_bytes(SourceName::new("unit.nct"), text.as_bytes())
            .expect("valid source");
        let tree = parse(
            sources.get(id).expect("inserted source"),
            ParseGoal::V2CoreUnit,
        );
        Self { sources, tree }
    }
    /// Read a physical conformance source. Its expectation catalog remains in the corpus manifest.
    /// # Panics
    /// Panics if the requested repository fixture is unavailable or invalid UTF-8.
    #[must_use]
    pub fn corpus(name: &str) -> Self {
        let path = crate::repository_root()
            .join("development/compiler/tests/fixtures/value-bindings")
            .join(name);
        Self::new(&std::fs::read_to_string(path).expect("source fixture"))
    }
    /// # Panics
    /// Panics only if this fixture's internally paired immutable source/tree becomes inconsistent.
    #[must_use]
    pub fn syntax(&self) -> BoundSyntax<'_> {
        BoundSyntax::new(
            self.sources.get(self.tree.source()).expect("owned source"),
            &self.tree,
        )
        .expect("exact source/tree identity")
    }
}
