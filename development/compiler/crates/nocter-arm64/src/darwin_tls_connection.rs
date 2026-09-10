use std::fmt;

use nocter_runtime_contract::DarwinNetworkCallbackRole;

use crate::darwin_network_connection::{
    Arm64DarwinConnectionParameters, add_darwin_connection_create_target,
};
use crate::{
    Arm64DarwinBlockError, Arm64DarwinNetworkAdapterImports, Arm64DarwinNetworkConnectionError,
    Arm64DarwinNetworkConnectionTargets, Arm64DarwinTlsAdapterImports, Arm64DarwinTlsCallbackError,
    Arm64FunctionId, Arm64ProgramBuilder, Arm64ProgramError,
    add_darwin_pointer_capture_block_descriptor, add_darwin_tls_configuration_callback,
};

/// Adds the TLS-only connection constructor over the common connection owner lifecycle.
///
/// The constructor receives the same owner destination and numeric endpoint as plain TCP, plus
/// NUL-terminated server-name and optional ALPN pointers. Only this capability declares Security
/// imports and constructs the synchronous TLS-configuration callback.
///
/// # Errors
///
/// Propagates fixed Block, TLS callback, connection construction, and program-data failures.
pub fn add_darwin_tls_connection_create_target(
    program: &mut Arm64ProgramBuilder,
    network: &Arm64DarwinNetworkAdapterImports,
    connection: Arm64DarwinNetworkConnectionTargets,
) -> Result<Arm64FunctionId, Arm64DarwinTlsConnectionError> {
    let tls = Arm64DarwinTlsAdapterImports::declare(program)?;
    let callback = add_darwin_tls_configuration_callback(program, &tls)?;
    let block = add_darwin_pointer_capture_block_descriptor(
        program,
        DarwinNetworkCallbackRole::ConfigureProtocol.block_signature(),
    )?;
    let queue_label = program.add_data(b"nocter.network.tls.connection\0".as_slice(), 1)?;
    add_darwin_connection_create_target(
        program,
        network,
        connection.state_callback(),
        connection.state_block(),
        queue_label,
        Arm64DarwinConnectionParameters::Tls { callback, block },
    )
    .map_err(Into::into)
}

#[derive(Debug)]
pub enum Arm64DarwinTlsConnectionError {
    Block(Arm64DarwinBlockError),
    Callback(Arm64DarwinTlsCallbackError),
    Connection(Arm64DarwinNetworkConnectionError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinTlsConnectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin TLS connection failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinTlsConnectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Block(error) => Some(error),
            Self::Callback(error) => Some(error),
            Self::Connection(error) => Some(error),
            Self::Program(error) => Some(error),
        }
    }
}

impl From<Arm64DarwinBlockError> for Arm64DarwinTlsConnectionError {
    fn from(error: Arm64DarwinBlockError) -> Self {
        Self::Block(error)
    }
}

impl From<Arm64DarwinTlsCallbackError> for Arm64DarwinTlsConnectionError {
    fn from(error: Arm64DarwinTlsCallbackError) -> Self {
        Self::Callback(error)
    }
}

impl From<Arm64DarwinNetworkConnectionError> for Arm64DarwinTlsConnectionError {
    fn from(error: Arm64DarwinNetworkConnectionError) -> Self {
        Self::Connection(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinTlsConnectionError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::RuntimeLibraryIdentity;

    use super::add_darwin_tls_connection_create_target;
    use crate::{
        Arm64DarwinNetworkAdapterImports, Arm64ProgramBuilder, add_darwin_plain_connection_targets,
    };

    #[test]
    fn secure_creation_extends_one_plain_connection_family_with_security_imports() {
        let mut program = Arm64ProgramBuilder::new();
        let network = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        let connection = add_darwin_plain_connection_targets(&mut program, &network).unwrap();
        let secure =
            add_darwin_tls_connection_create_target(&mut program, &network, connection).unwrap();
        assert_ne!(secure, connection.create());

        let entry = program.declare_function();
        let mut code = crate::Arm64CodeBuilder::new();
        code.append(crate::Arm64Instruction::BranchRegister {
            target: crate::Arm64Register::new(30).unwrap(),
            link: false,
        });
        program
            .define_function(entry, code.finish().unwrap())
            .unwrap();
        program.set_entry(entry).unwrap();
        let program = program.finish().unwrap();
        assert!(
            program.runtime_imports().iter().any(|import| {
                import.import().library() == RuntimeLibraryIdentity::DarwinSecurity
            })
        );
    }

    #[test]
    fn plain_creation_does_not_declare_security_imports() {
        let mut program = Arm64ProgramBuilder::new();
        let network = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        add_darwin_plain_connection_targets(&mut program, &network).unwrap();

        let entry = program.declare_function();
        let mut code = crate::Arm64CodeBuilder::new();
        code.append(crate::Arm64Instruction::BranchRegister {
            target: crate::Arm64Register::new(30).unwrap(),
            link: false,
        });
        program
            .define_function(entry, code.finish().unwrap())
            .unwrap();
        program.set_entry(entry).unwrap();
        let program = program.finish().unwrap();
        assert!(
            program.runtime_imports().iter().all(|import| {
                import.import().library() != RuntimeLibraryIdentity::DarwinSecurity
            })
        );
    }
}
