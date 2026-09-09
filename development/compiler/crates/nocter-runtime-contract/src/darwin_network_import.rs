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
    DispatchRetain,
    DispatchRelease,
    DispatchDataCreate,
    DispatchDataCreateMap,
    NetworkRetain,
    NetworkRelease,
    EndpointCreateHost,
    EndpointCreateAddress,
    EndpointGetAddress,
    ParametersCreateSecureTcp,
    ParametersSetLocalEndpoint,
    ConnectionCreate,
    ConnectionCopyCurrentPath,
    ConnectionSetStateHandler,
    ConnectionSetQueue,
    ConnectionStart,
    ConnectionCancel,
    ConnectionReceive,
    ConnectionSend,
    PathCopyEffectiveLocalEndpoint,
    PathCopyEffectiveRemoteEndpoint,
    ListenerCreate,
    ListenerSetQueue,
    ListenerSetStateHandler,
    ListenerSetNewConnectionHandler,
    ListenerStart,
    ListenerCancel,
    ListenerGetPort,
    NetworkErrorGetDomain,
    NetworkErrorGetCode,
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
        Self::DispatchRetain,
        Self::DispatchRelease,
        Self::DispatchDataCreate,
        Self::DispatchDataCreateMap,
        Self::NetworkRetain,
        Self::NetworkRelease,
        Self::EndpointCreateHost,
        Self::EndpointCreateAddress,
        Self::EndpointGetAddress,
        Self::ParametersCreateSecureTcp,
        Self::ParametersSetLocalEndpoint,
        Self::ConnectionCreate,
        Self::ConnectionCopyCurrentPath,
        Self::ConnectionSetStateHandler,
        Self::ConnectionSetQueue,
        Self::ConnectionStart,
        Self::ConnectionCancel,
        Self::ConnectionReceive,
        Self::ConnectionSend,
        Self::PathCopyEffectiveLocalEndpoint,
        Self::PathCopyEffectiveRemoteEndpoint,
        Self::ListenerCreate,
        Self::ListenerSetQueue,
        Self::ListenerSetStateHandler,
        Self::ListenerSetNewConnectionHandler,
        Self::ListenerStart,
        Self::ListenerCancel,
        Self::ListenerGetPort,
        Self::NetworkErrorGetDomain,
        Self::NetworkErrorGetCode,
    ];

    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive role-to-loader mapping is the import catalog's authority"
    )]
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
            Self::DispatchRetain => (RuntimeLibraryIdentity::DarwinSystem, "_dispatch_retain"),
            Self::DispatchRelease => (RuntimeLibraryIdentity::DarwinSystem, "_dispatch_release"),
            Self::DispatchDataCreate => (
                RuntimeLibraryIdentity::DarwinSystem,
                "_dispatch_data_create",
            ),
            Self::DispatchDataCreateMap => (
                RuntimeLibraryIdentity::DarwinSystem,
                "_dispatch_data_create_map",
            ),
            Self::NetworkRetain => (RuntimeLibraryIdentity::DarwinNetwork, "_nw_retain"),
            Self::NetworkRelease => (RuntimeLibraryIdentity::DarwinNetwork, "_nw_release"),
            Self::EndpointCreateHost => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_endpoint_create_host",
            ),
            Self::EndpointCreateAddress => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_endpoint_create_address",
            ),
            Self::EndpointGetAddress => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_endpoint_get_address",
            ),
            Self::ParametersCreateSecureTcp => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_parameters_create_secure_tcp",
            ),
            Self::ParametersSetLocalEndpoint => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_parameters_set_local_endpoint",
            ),
            Self::ConnectionCreate => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_connection_create",
            ),
            Self::ConnectionCopyCurrentPath => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_connection_copy_current_path",
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
            Self::ConnectionReceive => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_connection_receive",
            ),
            Self::ConnectionSend => (RuntimeLibraryIdentity::DarwinNetwork, "_nw_connection_send"),
            Self::PathCopyEffectiveLocalEndpoint => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_path_copy_effective_local_endpoint",
            ),
            Self::PathCopyEffectiveRemoteEndpoint => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_path_copy_effective_remote_endpoint",
            ),
            Self::ListenerCreate => (RuntimeLibraryIdentity::DarwinNetwork, "_nw_listener_create"),
            Self::ListenerSetQueue => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_listener_set_queue",
            ),
            Self::ListenerSetStateHandler => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_listener_set_state_changed_handler",
            ),
            Self::ListenerSetNewConnectionHandler => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_listener_set_new_connection_handler",
            ),
            Self::ListenerStart => (RuntimeLibraryIdentity::DarwinNetwork, "_nw_listener_start"),
            Self::ListenerCancel => (RuntimeLibraryIdentity::DarwinNetwork, "_nw_listener_cancel"),
            Self::ListenerGetPort => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_listener_get_port",
            ),
            Self::NetworkErrorGetDomain => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_error_get_error_domain",
            ),
            Self::NetworkErrorGetCode => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "_nw_error_get_error_code",
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
    DisableProtocolConfiguration,
    DefaultMessageContext,
    FinalMessageContext,
}

impl DarwinNetworkAdapterData {
    pub const ALL: &'static [Self] = &[
        Self::StackBlockClass,
        Self::DefaultProtocolConfiguration,
        Self::DisableProtocolConfiguration,
        Self::DefaultMessageContext,
        Self::FinalMessageContext,
    ];

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
            Self::DisableProtocolConfiguration => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "__nw_parameters_configure_protocol_disable",
            ),
            Self::DefaultMessageContext => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "__nw_content_context_default_message",
            ),
            Self::FinalMessageContext => (
                RuntimeLibraryIdentity::DarwinNetwork,
                "__nw_content_context_final_send",
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
