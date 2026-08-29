use std::collections::HashMap;
use std::fmt;

/// A compile-unit-local identifier spelling.
///
/// Declaration symbols are assigned by lexical byte order after deduplication, not discovery
/// order. A checking branch may append one lexically ordered body-only extension while preserving
/// every declaration symbol ID. Semantic type identity uses declaration IDs selected through
/// lookup rather than a `Symbol`.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Symbol(usize);

impl fmt::Debug for Symbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Symbol({})", self.0)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SymbolTable {
    spellings: Box<[Box<str>]>,
    by_spelling: HashMap<Box<str>, Symbol>,
}

impl SymbolTable {
    #[must_use]
    pub fn from_spellings<S>(spellings: impl IntoIterator<Item = S>) -> Self
    where
        S: AsRef<str>,
    {
        let mut spellings: Vec<Box<str>> = spellings
            .into_iter()
            .map(|spelling| spelling.as_ref().into())
            .collect();
        spellings.sort_unstable();
        spellings.dedup();
        Self::from_ordered(spellings)
    }

    #[must_use]
    pub fn get(&self, spelling: &str) -> Option<Symbol> {
        self.by_spelling.get(spelling).copied()
    }

    #[must_use]
    pub fn spelling(&self, symbol: Symbol) -> Option<&str> {
        self.spellings.get(symbol.0).map(AsRef::as_ref)
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (Symbol, &str)> {
        self.spellings
            .iter()
            .enumerate()
            .map(|(index, spelling)| (Symbol(index), spelling.as_ref()))
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.spellings.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.spellings.is_empty()
    }

    /// Appends one deterministic symbol extension without renumbering the existing domain.
    #[must_use]
    pub fn extended<S>(&self, spellings: impl IntoIterator<Item = S>) -> Self
    where
        S: AsRef<str>,
    {
        let mut extension: Vec<Box<str>> = spellings
            .into_iter()
            .map(|spelling| spelling.as_ref().into())
            .collect();
        extension.retain(|spelling| !self.by_spelling.contains_key(spelling.as_ref()));
        extension.sort_unstable();
        extension.dedup();
        let mut combined = self.spellings.to_vec();
        combined.extend(extension);
        Self::from_ordered(combined)
    }

    fn from_ordered(spellings: Vec<Box<str>>) -> Self {
        let by_spelling = spellings
            .iter()
            .enumerate()
            .map(|(index, spelling)| (spelling.clone(), Symbol(index)))
            .collect();
        Self {
            spellings: spellings.into_boxed_slice(),
            by_spelling,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SymbolTable;

    #[test]
    fn symbol_identity_does_not_depend_on_discovery_order() {
        let forward = SymbolTable::from_spellings(["value", "alpha", "value"]);
        let reverse = SymbolTable::from_spellings(["value", "alpha"].into_iter().rev());

        assert_eq!(forward, reverse);
        assert_eq!(forward.get("alpha"), reverse.get("alpha"));
        assert_eq!(forward.get("value"), reverse.get("value"));
        assert_eq!(
            forward.spelling(forward.get("value").unwrap()),
            Some("value")
        );
        assert_eq!(forward.get("missing"), None);
    }

    #[test]
    fn body_extension_preserves_the_declaration_prefix() {
        let declarations = SymbolTable::from_spellings(["Type", "method"]);
        let ty = declarations.get("Type").unwrap();
        let method = declarations.get("method").unwrap();

        let extended = declarations.extended(["local", "another", "Type"]);

        assert_eq!(extended.get("Type"), Some(ty));
        assert_eq!(extended.get("method"), Some(method));
        assert_eq!(extended.spelling(ty), Some("Type"));
        assert!(extended.get("local").is_some());
        assert!(extended.get("another").is_some());
    }
}
