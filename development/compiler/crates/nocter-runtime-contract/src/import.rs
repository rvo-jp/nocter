use std::fmt;

/// One operating-system library selected by a trusted runtime catalog.
///
/// This is an executable dependency identity, not a source-level import. The executable writer
/// owns its concrete path, load-command representation, and ordinal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLibraryIdentity {
    DarwinSystem,
    DarwinCoreFoundation,
    DarwinSecurity,
    DarwinNetwork,
}

/// One validated symbol identity in the selected target's loader namespace.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct RuntimeSymbolIdentity {
    library: RuntimeLibraryIdentity,
    symbol: Box<str>,
}

impl RuntimeSymbolIdentity {
    fn new(
        library: RuntimeLibraryIdentity,
        symbol: impl Into<Box<str>>,
    ) -> Result<Self, RuntimeImportError> {
        let symbol = symbol.into();
        let mut bytes = symbol.bytes();
        let Some(first) = bytes.next() else {
            return Err(RuntimeImportError::InvalidSymbol);
        };
        if !is_symbol_start(first) || !bytes.all(is_symbol_continue) {
            return Err(RuntimeImportError::InvalidSymbol);
        }
        Ok(Self { library, symbol })
    }
}

/// One exact external function required by a closed runtime call plan.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeFunctionImport(RuntimeSymbolIdentity);

impl RuntimeFunctionImport {
    pub(crate) fn trusted(library: RuntimeLibraryIdentity, symbol: &'static str) -> Self {
        Self(RuntimeSymbolIdentity {
            library,
            symbol: symbol.into(),
        })
    }

    /// Constructs one validated loader function.
    ///
    /// # Errors
    ///
    /// Rejects an empty symbol or a spelling outside the portable C identifier subset used by the
    /// trusted catalog.
    pub fn new(
        library: RuntimeLibraryIdentity,
        symbol: impl Into<Box<str>>,
    ) -> Result<Self, RuntimeImportError> {
        RuntimeSymbolIdentity::new(library, symbol).map(Self)
    }

    #[must_use]
    pub const fn library(&self) -> RuntimeLibraryIdentity {
        self.0.library
    }

    #[must_use]
    pub const fn symbol(&self) -> &str {
        &self.0.symbol
    }
}

/// One exact external data object required by a closed runtime adapter.
///
/// The loader slot contains the address of the external object. Reading the object's value, when
/// needed, is a distinct target operation and cannot be confused with calling a function slot.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeDataImport(RuntimeSymbolIdentity);

impl RuntimeDataImport {
    pub(crate) fn trusted(library: RuntimeLibraryIdentity, symbol: &'static str) -> Self {
        Self(RuntimeSymbolIdentity {
            library,
            symbol: symbol.into(),
        })
    }

    /// Constructs one validated loader data object.
    ///
    /// # Errors
    ///
    /// Rejects an empty symbol or a spelling outside the trusted loader-symbol subset.
    pub fn new(
        library: RuntimeLibraryIdentity,
        symbol: impl Into<Box<str>>,
    ) -> Result<Self, RuntimeImportError> {
        RuntimeSymbolIdentity::new(library, symbol).map(Self)
    }

    #[must_use]
    pub const fn library(&self) -> RuntimeLibraryIdentity {
        self.0.library
    }

    #[must_use]
    pub const fn symbol(&self) -> &str {
        &self.0.symbol
    }
}

/// The kind-preserving import retained by target code and consumed by an executable writer.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeImport {
    Function(RuntimeFunctionImport),
    Data(RuntimeDataImport),
}

impl RuntimeImport {
    #[must_use]
    pub const fn library(&self) -> RuntimeLibraryIdentity {
        match self {
            Self::Function(import) => import.library(),
            Self::Data(import) => import.library(),
        }
    }

    #[must_use]
    pub const fn symbol(&self) -> &str {
        match self {
            Self::Function(import) => import.symbol(),
            Self::Data(import) => import.symbol(),
        }
    }
}

impl From<RuntimeFunctionImport> for RuntimeImport {
    fn from(import: RuntimeFunctionImport) -> Self {
        Self::Function(import)
    }
}

impl From<RuntimeDataImport> for RuntimeImport {
    fn from(import: RuntimeDataImport) -> Self {
        Self::Data(import)
    }
}

const fn is_symbol_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

const fn is_symbol_continue(byte: u8) -> bool {
    is_symbol_start(byte) || byte.is_ascii_digit()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeImportError {
    InvalidSymbol,
}

impl fmt::Display for RuntimeImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("runtime import has an invalid loader symbol")
    }
}

impl std::error::Error for RuntimeImportError {}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeDataImport, RuntimeFunctionImport, RuntimeImport, RuntimeImportError,
        RuntimeLibraryIdentity,
    };

    #[test]
    fn loader_symbols_are_validated_once_and_retain_their_kind() {
        let function =
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, "_getaddrinfo")
                .unwrap();
        assert_eq!(function.library(), RuntimeLibraryIdentity::DarwinSystem);
        assert_eq!(function.symbol(), "_getaddrinfo");

        let data = RuntimeDataImport::new(
            RuntimeLibraryIdentity::DarwinNetwork,
            "_nw_parameters_configure_protocol_default_configuration",
        )
        .unwrap();
        assert_eq!(data.library(), RuntimeLibraryIdentity::DarwinNetwork);
        assert!(matches!(RuntimeImport::from(data), RuntimeImport::Data(_)));
        assert!(matches!(
            RuntimeImport::from(function),
            RuntimeImport::Function(_)
        ));

        assert_ne!(
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSecurity, "_SSLHandshake")
                .unwrap(),
            RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, "_SSLHandshake")
                .unwrap()
        );

        for invalid in ["", "get-address", "9invalid", "symbol\0tail"] {
            assert_eq!(
                RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, invalid),
                Err(RuntimeImportError::InvalidSymbol)
            );
            assert_eq!(
                RuntimeDataImport::new(RuntimeLibraryIdentity::DarwinSystem, invalid),
                Err(RuntimeImportError::InvalidSymbol)
            );
        }
    }
}
