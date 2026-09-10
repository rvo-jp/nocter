use crate::{RuntimeFunctionImport, RuntimeLibraryIdentity};

/// Function dependencies admitted only when the closed Darwin TLS adapter is selected.
///
/// Keeping this catalog separate from plain Network.framework imports prevents a plain TCP image
/// from acquiring a Security.framework load command merely because TLS exists in the toolchain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinTlsAdapterFunction {
    CopySecurityProtocolOptions,
    SetServerName,
    AddApplicationProtocol,
    SetMinimumProtocolVersion,
    SecurityRelease,
}

impl DarwinTlsAdapterFunction {
    pub const ALL: &'static [Self] = &[
        Self::CopySecurityProtocolOptions,
        Self::SetServerName,
        Self::AddApplicationProtocol,
        Self::SetMinimumProtocolVersion,
        Self::SecurityRelease,
    ];

    #[must_use]
    pub fn import(self) -> RuntimeFunctionImport {
        let (library, symbol) = match self {
            Self::CopySecurityProtocolOptions => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_tls_copy_sec_protocol_options",
            ),
            Self::SetServerName => (
                RuntimeLibraryIdentity::DarwinSecurity,
                "_sec_protocol_options_set_tls_server_name",
            ),
            Self::AddApplicationProtocol => (
                RuntimeLibraryIdentity::DarwinSecurity,
                "_sec_protocol_options_add_tls_application_protocol",
            ),
            Self::SetMinimumProtocolVersion => (
                RuntimeLibraryIdentity::DarwinSecurity,
                "_sec_protocol_options_set_min_tls_protocol_version",
            ),
            Self::SecurityRelease => (RuntimeLibraryIdentity::DarwinSecurity, "_sec_release"),
        };
        RuntimeFunctionImport::trusted(library, symbol)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::DarwinTlsAdapterFunction;
    use crate::{RuntimeFunctionImport, RuntimeLibraryIdentity};

    #[test]
    fn tls_imports_are_unique_and_keep_security_out_of_plain_network_catalogs() {
        let imports = DarwinTlsAdapterFunction::ALL
            .iter()
            .copied()
            .map(DarwinTlsAdapterFunction::import)
            .collect::<Vec<_>>();
        assert_eq!(imports.iter().collect::<BTreeSet<_>>().len(), imports.len());
        assert!(
            imports
                .iter()
                .any(|import| { import.library() == RuntimeLibraryIdentity::DarwinNetwork })
        );
        assert!(
            imports
                .iter()
                .any(|import| { import.library() == RuntimeLibraryIdentity::DarwinSecurity })
        );
        for import in imports {
            assert_eq!(
                RuntimeFunctionImport::new(import.library(), import.symbol()).unwrap(),
                import
            );
        }
    }
}
