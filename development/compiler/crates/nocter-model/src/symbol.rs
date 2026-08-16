use std::fmt;

/// A compile-unit-local identifier spelling.
///
/// Symbols are assigned by lexical byte order after deduplication, not discovery order. They are
/// suitable for lookup tables and presentation metadata, but semantic type identity uses the
/// declaration IDs selected through lookup rather than a `Symbol`.
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
        Self {
            spellings: spellings.into_boxed_slice(),
        }
    }

    #[must_use]
    pub fn get(&self, spelling: &str) -> Option<Symbol> {
        self.spellings
            .binary_search_by(|candidate| candidate.as_ref().cmp(spelling))
            .ok()
            .map(Symbol)
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
}
