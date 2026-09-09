use crate::{RuntimeDataImport, RuntimeFunctionImport, RuntimeLibraryIdentity};

/// One function dependency admitted by the closed Darwin Network adapter.
///
/// Target code consumes this role instead of spelling loader symbols or choosing their libraries.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinNetworkAdapterFunction {
    SocketPair,
    Send,
    Receive,
    Close,
    Abort,
    ErrorAddress,
    DispatchQueueCreate,
    DispatchAsync,
    DispatchSync,
    DispatchRelease,
    NetworkRetain,
    NetworkRelease,
    EndpointCreateHost,
    ParametersCreateSecureTcp,
    ConnectionCreate,
    ConnectionSetStateHandler,
    ConnectionSetQueue,
    ConnectionStart,
    ConnectionCancel,
}

impl DarwinNetworkAdapterFunction {
    pub const ALL: &'static [Self] = &[
        Self::SocketPair,
        Self::Send,
        Self::Receive,
        Self::Close,
        Self::Abort,
        Self::ErrorAddress,
        Self::DispatchQueueCreate,
        Self::DispatchAsync,
        Self::DispatchSync,
        Self::DispatchRelease,
        Self::NetworkRetain,
        Self::NetworkRelease,
        Self::EndpointCreateHost,
        Self::ParametersCreateSecureTcp,
        Self::ConnectionCreate,
        Self::ConnectionSetStateHandler,
        Self::ConnectionSetQueue,
        Self::ConnectionStart,
        Self::ConnectionCancel,
    ];

    #[must_use]
    pub fn import(self) -> RuntimeFunctionImport {
        let (library, symbol) = match self {
            Self::SocketPair => (RuntimeLibraryIdentity::DarwinSystem, "_socketpair"),
            Self::Send => (RuntimeLibraryIdentity::DarwinSystem, "_send"),
            Self::Receive => (RuntimeLibraryIdentity::DarwinSystem, "_recv"),
            Self::Close => (RuntimeLibraryIdentity::DarwinSystem, "_close"),
            Self::Abort => (RuntimeLibraryIdentity::DarwinSystem, "_abort"),
            Self::ErrorAddress => (RuntimeLibraryIdentity::DarwinSystem, "___error"),
            Self::DispatchQueueCreate => (
                RuntimeLibraryIdentity::DarwinSystem,
                "_dispatch_queue_create",
            ),
            Self::DispatchAsync => (RuntimeLibraryIdentity::DarwinSystem, "_dispatch_async"),
            Self::DispatchSync => (RuntimeLibraryIdentity::DarwinSystem, "_dispatch_sync_f"),
            Self::DispatchRelease => (RuntimeLibraryIdentity::DarwinSystem, "_dispatch_release"),
            Self::NetworkRetain => (RuntimeLibraryIdentity::DarwinNetwork, "_nw_retain"),
            Self::NetworkRelease => (RuntimeLibraryIdentity::DarwinNetwork, "_nw_release"),
            Self::EndpointCreateHost => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_endpoint_create_host",
            ),
            Self::ParametersCreateSecureTcp => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_parameters_create_secure_tcp",
            ),
            Self::ConnectionCreate => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_connection_create",
            ),
            Self::ConnectionSetStateHandler => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_connection_set_state_changed_handler",
            ),
            Self::ConnectionSetQueue => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_connection_set_queue",
            ),
            Self::ConnectionStart => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_connection_start",
            ),
            Self::ConnectionCancel => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_connection_cancel",
            ),
        };
        RuntimeFunctionImport::trusted(library, symbol)
    }
}

/// One data dependency admitted by the closed Darwin Network adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinNetworkAdapterData {
    StackBlockClass,
    DefaultProtocolConfiguration,
}

impl DarwinNetworkAdapterData {
    pub const ALL: &'static [Self] = &[Self::StackBlockClass, Self::DefaultProtocolConfiguration];

    #[must_use]
    pub fn import(self) -> RuntimeDataImport {
        let (library, symbol) = match self {
            Self::StackBlockClass => (
                RuntimeLibraryIdentity::DarwinSystem,
                "__NSConcreteStackBlock",
            ),
            Self::DefaultProtocolConfiguration => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "__nw_parameters_configure_protocol_default_configuration",
            ),
        };
        RuntimeDataImport::trusted(library, symbol)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{DarwinNetworkAdapterData, DarwinNetworkAdapterFunction};
    use crate::{RuntimeDataImport, RuntimeFunctionImport, RuntimeImport};

    #[test]
    fn adapter_catalog_preserves_kind_and_unique_loader_identity() {
        let imports = DarwinNetworkAdapterFunction::ALL
            .iter()
            .copied()
            .map(|role| RuntimeImport::from(role.import()))
            .chain(
                DarwinNetworkAdapterData::ALL
                    .iter()
                    .copied()
                    .map(|role| RuntimeImport::from(role.import())),
            )
            .collect::<Vec<_>>();
        assert_eq!(imports.iter().collect::<BTreeSet<_>>().len(), imports.len());
        assert!(
            imports[..DarwinNetworkAdapterFunction::ALL.len()]
                .iter()
                .all(|import| matches!(import, RuntimeImport::Function(_)))
        );
        assert!(
            imports[DarwinNetworkAdapterFunction::ALL.len()..]
                .iter()
                .all(|import| matches!(import, RuntimeImport::Data(_)))
        );
        for role in DarwinNetworkAdapterFunction::ALL {
            let import = role.import();
            assert_eq!(
                RuntimeFunctionImport::new(import.library(), import.symbol()).unwrap(),
                import
            );
        }
        for role in DarwinNetworkAdapterData::ALL {
            let import = role.import();
            assert_eq!(
                RuntimeDataImport::new(import.library(), import.symbol()).unwrap(),
                import
            );
        }
    }
}
