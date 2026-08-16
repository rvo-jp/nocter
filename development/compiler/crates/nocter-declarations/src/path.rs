use nocter_model::Symbol;

/// A normalized directory-module path relative to its package root.
///
/// The root module has no segments. Lowering resolves `.` and `..` before constructing this value,
/// so navigation spellings cannot enter module identity.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModulePath(Box<[Symbol]>);

impl ModulePath {
    #[must_use]
    pub fn root() -> Self {
        Self(Box::new([]))
    }

    #[must_use]
    pub fn from_segments(segments: impl Into<Box<[Symbol]>>) -> Self {
        Self(segments.into())
    }

    #[must_use]
    pub const fn segments(&self) -> &[Symbol] {
        &self.0
    }

    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn is_ancestor_of(&self, other: &Self) -> bool {
        other.0.starts_with(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::SymbolTable;

    use super::ModulePath;

    #[test]
    fn root_and_directory_ancestry_are_structural() {
        let symbols = SymbolTable::from_spellings(["parser", "lexer"]);
        let parser = symbols.get("parser").unwrap();
        let lexer = symbols.get("lexer").unwrap();
        let root = ModulePath::root();
        let parser_path = ModulePath::from_segments([parser]);
        let lexer_path = ModulePath::from_segments([parser, lexer]);

        assert!(root.is_ancestor_of(&lexer_path));
        assert!(parser_path.is_ancestor_of(&lexer_path));
        assert!(!lexer_path.is_ancestor_of(&parser_path));
    }
}
