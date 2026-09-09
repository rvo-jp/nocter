use std::fmt;

/// One operating-system library selected by a trusted target-service catalog.
///
/// This is an executable dependency identity, not a source-level import. The executable writer
/// owns its concrete path and load-command representation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLibraryIdentity {
    DarwinSystem,
    DarwinCoreFoundation,
    DarwinSecurity,
}

/// One exact external function required by a closed runtime call plan.
///
/// Symbols are already expressed in the selected target's loader namespace. No executable stage
/// derives a symbol from a Nocter declaration name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeFunctionImport {
    library: RuntimeLibraryIdentity,
    symbol: Box<str>,
}

impl RuntimeFunctionImport {
    /// Constructs one validated loader symbol.
    ///
    /// # Errors
    ///
    /// Rejects an empty symbol or a spelling outside the portable C identifier subset used by the
    /// trusted catalog.
    pub fn new(
        library: RuntimeLibraryIdentity,
        symbol: impl Into<Box<str>>,
    ) -> Result<Self, RuntimeFunctionImportError> {
        let symbol = symbol.into();
        let mut bytes = symbol.bytes();
        let Some(first) = bytes.next() else {
            return Err(RuntimeFunctionImportError::InvalidSymbol);
        };
        if !is_symbol_start(first) || !bytes.all(is_symbol_continue) {
            return Err(RuntimeFunctionImportError::InvalidSymbol);
        }
        Ok(Self { library, symbol })
    }

    #[must_use]
    pub const fn library(&self) -> RuntimeLibraryIdentity {
        self.library
    }

    #[must_use]
    pub const fn symbol(&self) -> &str {
        &self.symbol
    }
}

const fn is_symbol_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

const fn is_symbol_continue(byte: u8) -> bool {
    is_symbol_start(byte) || byte.is_ascii_digit()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeFunctionImportError {
    InvalidSymbol,
}

impl fmt::Display for RuntimeFunctionImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("runtime function import has an invalid loader symbol")
    }
}

impl std::error::Error for RuntimeFunctionImportError {}

#[cfg(test)]
mod tests {
    use super::{RuntimeFunctionImport, RuntimeFunctionImportError, RuntimeLibraryIdentity};

    #[test]
    fn loader_symbols_are_validated_once() {
        let import =
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, "_getaddrinfo")
                .unwrap();
        assert_eq!(import.library(), RuntimeLibraryIdentity::DarwinSystem);
        assert_eq!(import.symbol(), "_getaddrinfo");

        assert_ne!(
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSecurity, "_SSLHandshake")
                .unwrap(),
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, "_SSLHandshake")
                .unwrap()
        );

        for invalid in ["", "get-address", "9invalid", "symbol\0tail"] {
            assert_eq!(
                RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, invalid),
                Err(RuntimeFunctionImportError::InvalidSymbol)
            );
        }
    }
}
