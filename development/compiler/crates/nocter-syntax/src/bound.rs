use nocter_source::{SourceFile, TextRange};

use crate::SyntaxTree;

/// A source file and syntax tree proven to describe the same exact source identity.
///
/// Source-backed syntax consumers accept this value instead of trusting independently supplied
/// references. `SourceId` carries an opaque issuance identity, so successful construction also
/// rejects a stale tree whose numeric source index happens to match a newer compiler snapshot.
#[derive(Clone, Copy, Debug)]
pub struct BoundSyntax<'syntax> {
    source: &'syntax SourceFile,
    tree: &'syntax SyntaxTree,
}

impl<'syntax> BoundSyntax<'syntax> {
    #[must_use]
    pub fn new(source: &'syntax SourceFile, tree: &'syntax SyntaxTree) -> Option<Self> {
        (source.id() == tree.source()).then_some(Self { source, tree })
    }

    #[must_use]
    pub const fn source(self) -> &'syntax SourceFile {
        self.source
    }

    #[must_use]
    pub const fn tree(self) -> &'syntax SyntaxTree {
        self.tree
    }

    #[must_use]
    pub fn text_at(self, range: TextRange) -> Option<&'syntax str> {
        self.source.text_at(range)
    }
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};

    use crate::{BoundSyntax, ParseGoal, parse};

    #[test]
    fn independent_source_maps_cannot_form_a_bound_syntax_pair() {
        let mut first = SourceMap::new();
        let first_id = first
            .add_bytes(SourceName::new("first.nct"), b"func one(): void")
            .unwrap();
        let tree = parse(first.get(first_id).unwrap(), ParseGoal::SourceFile);
        let mut second = SourceMap::new();
        let second_id = second
            .add_bytes(SourceName::new("second.nct"), b"func two(): void")
            .unwrap();

        assert!(BoundSyntax::new(first.get(first_id).unwrap(), &tree).is_some());
        assert!(BoundSyntax::new(second.get(second_id).unwrap(), &tree).is_none());
    }

    #[test]
    fn node_identity_cannot_cross_independent_trees_for_the_same_source() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("source.nct"), b"func value(): void")
            .unwrap();
        let file = sources.get(source).unwrap();
        let first = parse(file, ParseGoal::SourceFile);
        let second = parse(file, ParseGoal::SourceFile);
        let cloned = first.clone();

        assert_ne!(first.root_id(), second.root_id());
        assert!(first.node(second.root_id()).is_none());
        assert_eq!(first.root_id(), cloned.root_id());
        assert!(cloned.node(first.root_id()).is_some());
        assert_eq!(
            std::mem::size_of::<crate::NodeId>(),
            std::mem::size_of::<[u64; 2]>()
        );
    }
}
