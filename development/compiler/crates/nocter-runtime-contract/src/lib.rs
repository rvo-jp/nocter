//! Source-independent contracts shared across target closure, MIR, and machine lowering.
//!
//! This crate owns identities and immutable representation facts, not the implementation that
//! discovers, validates, lowers, lays out, or executes them. It depends only on semantic model
//! identities so no backend consumer gains access to checked or target-program storage.

mod asynchronous;
mod darwin_block;
mod darwin_network;
mod darwin_network_address;
mod darwin_network_import;
mod darwin_network_operation;
mod darwin_network_owner;
mod darwin_tls;
mod darwin_tls_import;
mod environment;
mod import;
mod primitive;
mod representation;
mod runtime_storage;
mod target;
mod target_service;

pub use asynchronous::{ReactorInterest, ReadinessDirection};
pub use darwin_block::DarwinBlockAbiSchema;
pub use darwin_network::{
    DarwinNetworkCallbackEventAbiSchema, DarwinNetworkCallbackRole, DarwinNetworkChannelIoContract,
    DarwinNetworkChannelIoOutcome, DarwinNetworkConnectionEventObservationAbiSchema,
    DarwinNetworkConnectionState, DarwinNetworkEventKind, DarwinNetworkEventPayload,
    DarwinNetworkListenerEventObservationAbiSchema, DarwinNetworkListenerState,
};
pub use darwin_network_address::DarwinNetworkSocketAddressAbiSchema;
pub use darwin_network_import::{DarwinNetworkAdapterData, DarwinNetworkAdapterFunction};
pub use darwin_network_operation::{
    DarwinNetworkAdapterOperation, DarwinNetworkOperationError, DarwinNetworkOwner,
    DarwinNetworkOwnerKind, DarwinNetworkOwnerState,
};
pub use darwin_network_owner::{
    DarwinNetworkOwnerAbiSchema, DarwinNetworkOwnerCreateStatus, DarwinNetworkOwnerField,
    DarwinNetworkOwnerResourceFamily,
};
pub use darwin_tls::DarwinTlsConfigurationAbiSchema;
pub use darwin_tls_import::DarwinTlsAdapterFunction;
pub use environment::{RuntimeEnvironment, RuntimeEnvironmentError};
pub use import::{
    RuntimeDataImport, RuntimeFunctionImport, RuntimeImport, RuntimeImportError,
    RuntimeLibraryIdentity,
};
pub use primitive::{
    PrimitiveBinding, PrimitiveBindingError, PrimitiveEffects, PrimitiveRegistry, PrimitiveRole,
};
pub use representation::{
    RuntimeCaptureRepresentation, RuntimeFieldRepresentation, RuntimePayloadRepresentation,
    RuntimeTypeRepresentation, RuntimeTypeRepresentationTable, RuntimeVariantRepresentation,
};
pub use runtime_storage::{
    RuntimeStorageBinding, RuntimeStorageBindingError, RuntimeStorageLayout,
    RuntimeStorageRegistry, RuntimeStorageRole,
};
pub use target_service::{
    TargetServiceBinding, TargetServiceBindingError, TargetServiceCallingConvention,
    TargetServiceDescriptor, TargetServiceRegistry, TargetServiceRole, TargetServiceSignature,
    TargetServiceValueAbi,
};
pub use types::{
    RuntimePrimitive, RuntimeType, RuntimeTypeTable, RuntimeTypeTableBuildError,
    RuntimeTypeTableBuilder,
};
mod types;
pub use target::{
    RuntimeAbiIdentity, RuntimeAbiSchema, RuntimeAsyncAbiSchema, RuntimeAsyncStateTags,
    RuntimeEndianness, RuntimeErrorAbiSchema, RuntimeWaitInterestRecord,
};
