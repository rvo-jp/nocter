use crate::{RuntimeDataImport, RuntimeFunctionImport, RuntimeLibraryIdentity};

/// Function dependencies admitted only when the closed Darwin TLS adapter is selected.
///
/// Keeping this catalog separate from plain Network.framework imports prevents a plain TCP image
/// from acquiring a Security.framework load command merely because TLS exists in the toolchain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinTlsAdapterFunction {
    Malloc,
    MemoryCopy,
    CopySecurityProtocolOptions,
    SetServerName,
    AddApplicationProtocol,
    SetMinimumProtocolVersion,
    SetVerifyBlock,
    CopyTrustReference,
    CoreFoundationDataCreate,
    SecurityCertificateCreateWithData,
    CoreFoundationArrayCreate,
    SecurityTrustSetAnchorCertificates,
    SecurityTrustSetAnchorCertificatesOnly,
    SecurityTrustEvaluateWithError,
    CoreFoundationRelease,
    SecurityRelease,
}

/// Data dependencies admitted only by the Darwin TLS adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinTlsAdapterData {
    CoreFoundationTypeArrayCallbacks,
}

impl DarwinTlsAdapterData {
    pub const ALL: &'static [Self] = &[Self::CoreFoundationTypeArrayCallbacks];

    #[must_use]
    pub fn import(self) -> RuntimeDataImport {
        match self {
            Self::CoreFoundationTypeArrayCallbacks => RuntimeDataImport::trusted(
                RuntimeLibraryIdentity::DarwinCoreFoundation,
                "_kCFTypeArrayCallBacks",
            ),
        }
    }
}

impl DarwinTlsAdapterFunction {
    pub const ALL: &'static [Self] = &[
        Self::Malloc,
        Self::MemoryCopy,
        Self::CopySecurityProtocolOptions,
        Self::SetServerName,
        Self::AddApplicationProtocol,
        Self::SetMinimumProtocolVersion,
        Self::SetVerifyBlock,
        Self::CopyTrustReference,
        Self::CoreFoundationDataCreate,
        Self::SecurityCertificateCreateWithData,
        Self::CoreFoundationArrayCreate,
        Self::SecurityTrustSetAnchorCertificates,
        Self::SecurityTrustSetAnchorCertificatesOnly,
        Self::SecurityTrustEvaluateWithError,
        Self::CoreFoundationRelease,
        Self::SecurityRelease,
    ];

    #[must_use]
    pub fn import(self) -> RuntimeFunctionImport {
        let (library, symbol) = match self {
            Self::Malloc => (RuntimeLibraryIdentity::DarwinSystem, "_malloc"),
            Self::MemoryCopy => (RuntimeLibraryIdentity::DarwinSystem, "_memcpy"),
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
            Self::SetVerifyBlock => (
                RuntimeLibraryIdentity::DarwinSecurity,
                "_sec_protocol_options_set_verify_block",
            ),
            Self::CopyTrustReference => (
                RuntimeLibraryIdentity::DarwinSecurity,
                "_sec_trust_copy_ref",
            ),
            Self::CoreFoundationDataCreate => (
                RuntimeLibraryIdentity::DarwinCoreFoundation,
                "_CFDataCreate",
            ),
            Self::SecurityCertificateCreateWithData => (
                RuntimeLibraryIdentity::DarwinSecurity,
                "_SecCertificateCreateWithData",
            ),
            Self::CoreFoundationArrayCreate => (
                RuntimeLibraryIdentity::DarwinCoreFoundation,
                "_CFArrayCreate",
            ),
            Self::SecurityTrustSetAnchorCertificates => (
                RuntimeLibraryIdentity::DarwinSecurity,
                "_SecTrustSetAnchorCertificates",
            ),
            Self::SecurityTrustSetAnchorCertificatesOnly => (
                RuntimeLibraryIdentity::DarwinSecurity,
                "_SecTrustSetAnchorCertificatesOnly",
            ),
            Self::SecurityTrustEvaluateWithError => (
                RuntimeLibraryIdentity::DarwinSecurity,
                "_SecTrustEvaluateWithError",
            ),
            Self::CoreFoundationRelease => {
                (RuntimeLibraryIdentity::DarwinCoreFoundation, "_CFRelease")
            }
            Self::SecurityRelease => (RuntimeLibraryIdentity::DarwinSecurity, "_sec_release"),
        };
        RuntimeFunctionImport::trusted(library, symbol)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{DarwinTlsAdapterData, DarwinTlsAdapterFunction};
    use crate::{RuntimeDataImport, RuntimeFunctionImport, RuntimeLibraryIdentity};

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
        let data = DarwinTlsAdapterData::CoreFoundationTypeArrayCallbacks.import();
        assert_eq!(data.library(), RuntimeLibraryIdentity::DarwinCoreFoundation);
        assert_eq!(
            RuntimeDataImport::new(data.library(), data.symbol()).unwrap(),
            data
        );
    }
}
