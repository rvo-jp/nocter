use nocter_runtime_contract::{DarwinTlsAdapterData, DarwinTlsAdapterFunction};

use crate::{Arm64DataImportId, Arm64FunctionImportId, Arm64ProgramBuilder, Arm64ProgramError};

/// Capability-scoped loader dependencies for TLS configuration.
///
/// This value is declared only when a secure connection target exists. The ordinary network
/// adapter therefore remains independent of Security.framework.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64DarwinTlsAdapterImports {
    functions: Box<[(DarwinTlsAdapterFunction, Arm64FunctionImportId)]>,
    data: Box<[(DarwinTlsAdapterData, Arm64DataImportId)]>,
}

impl Arm64DarwinTlsAdapterImports {
    /// Declares the complete TLS-only catalog in runtime authority order.
    ///
    /// # Errors
    ///
    /// Propagates program import-table construction failures.
    pub fn declare(program: &mut Arm64ProgramBuilder) -> Result<Self, Arm64ProgramError> {
        let functions = DarwinTlsAdapterFunction::ALL
            .iter()
            .copied()
            .map(|role| Ok((role, program.add_function_import(role.import())?)))
            .collect::<Result<Vec<_>, Arm64ProgramError>>()?
            .into_boxed_slice();
        let data = DarwinTlsAdapterData::ALL
            .iter()
            .copied()
            .map(|role| Ok((role, program.add_data_import(role.import())?)))
            .collect::<Result<Vec<_>, Arm64ProgramError>>()?
            .into_boxed_slice();
        Ok(Self { functions, data })
    }

    /// Returns the unique loader slot assigned to one TLS role.
    ///
    /// # Panics
    ///
    /// Only if this privately constructed value violates its complete-catalog invariant.
    #[must_use]
    pub fn function(&self, role: DarwinTlsAdapterFunction) -> Arm64FunctionImportId {
        self.functions
            .iter()
            .find_map(|(candidate, target)| (*candidate == role).then_some(*target))
            .expect("complete Darwin TLS function catalog")
    }

    /// Returns the unique loader slot assigned to one TLS data role.
    ///
    /// # Panics
    ///
    /// Only if this privately constructed value violates its complete-catalog invariant.
    #[must_use]
    pub fn data(&self, role: DarwinTlsAdapterData) -> Arm64DataImportId {
        self.data
            .iter()
            .find_map(|(candidate, target)| (*candidate == role).then_some(*target))
            .expect("complete Darwin TLS data catalog")
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::{
        DarwinTlsAdapterData, DarwinTlsAdapterFunction, RuntimeImport, RuntimeLibraryIdentity,
    };

    use super::Arm64DarwinTlsAdapterImports;
    use crate::{Arm64CodeBuilder, Arm64Instruction, Arm64ProgramBuilder, Arm64Register};

    #[test]
    fn tls_imports_are_declared_only_by_the_tls_capability() {
        let mut program = Arm64ProgramBuilder::new();
        let first = Arm64DarwinTlsAdapterImports::declare(&mut program).unwrap();
        let second = Arm64DarwinTlsAdapterImports::declare(&mut program).unwrap();
        assert_eq!(first, second);
        for role in DarwinTlsAdapterFunction::ALL {
            assert_eq!(first.function(*role), second.function(*role));
        }
        for role in DarwinTlsAdapterData::ALL {
            assert_eq!(first.data(*role), second.data(*role));
        }

        let entry = program.declare_function();
        let mut code = Arm64CodeBuilder::new();
        code.append(Arm64Instruction::BranchRegister {
            target: Arm64Register::new(30).unwrap(),
            link: false,
        });
        program
            .define_function(entry, code.finish().unwrap())
            .unwrap();
        program.set_entry(entry).unwrap();
        let program = program.finish().unwrap();
        assert!(
            program.runtime_imports().iter().any(|entry| {
                entry.import().library() == RuntimeLibraryIdentity::DarwinSecurity
            })
        );
        assert_eq!(
            program
                .runtime_imports()
                .iter()
                .map(|entry| entry.import().clone())
                .collect::<Vec<_>>(),
            DarwinTlsAdapterFunction::ALL
                .iter()
                .copied()
                .map(|role| RuntimeImport::from(role.import()))
                .chain(
                    DarwinTlsAdapterData::ALL
                        .iter()
                        .copied()
                        .map(|role| RuntimeImport::from(role.import())),
                )
                .collect::<Vec<_>>()
        );
    }
}
