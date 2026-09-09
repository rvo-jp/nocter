use nocter_runtime_contract::{DarwinNetworkAdapterData, DarwinNetworkAdapterFunction};

use crate::{
    Arm64DarwinNetworkChannelImports, Arm64DataImportId, Arm64FunctionImportId,
    Arm64ProgramBuilder, Arm64ProgramError,
};

/// Complete typed loader dependencies for the closed Darwin network adapter.
///
/// The runtime catalog remains the authority for symbol spelling, symbol kind, and library. This
/// target view assigns ARM64 pointer slots once and prevents adapter emitters from reconstructing
/// or selectively substituting those decisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64DarwinNetworkAdapterImports {
    channel: Arm64DarwinNetworkChannelImports,
    functions: Box<[(DarwinNetworkAdapterFunction, Arm64FunctionImportId)]>,
    data: Box<[(DarwinNetworkAdapterData, Arm64DataImportId)]>,
}

impl Arm64DarwinNetworkAdapterImports {
    /// Declares every dependency in canonical runtime-role order.
    ///
    /// [`Arm64ProgramBuilder`] deduplicates the subsequently requested channel subset against the
    /// complete function catalog, so callback and operation emitters necessarily share the same
    /// pointer slots without perturbing canonical declaration order.
    ///
    /// # Errors
    ///
    /// Propagates an ARM64 program data or alignment failure.
    pub fn declare(program: &mut Arm64ProgramBuilder) -> Result<Self, Arm64ProgramError> {
        let functions = DarwinNetworkAdapterFunction::ALL
            .iter()
            .copied()
            .map(|role| Ok((role, program.add_function_import(role.import())?)))
            .collect::<Result<Vec<_>, Arm64ProgramError>>()?
            .into_boxed_slice();
        let channel = Arm64DarwinNetworkChannelImports::declare(program)?;
        let data = DarwinNetworkAdapterData::ALL
            .iter()
            .copied()
            .map(|role| Ok((role, program.add_data_import(role.import())?)))
            .collect::<Result<Vec<_>, Arm64ProgramError>>()?
            .into_boxed_slice();
        Ok(Self {
            channel,
            functions,
            data,
        })
    }

    #[must_use]
    pub const fn channel(&self) -> Arm64DarwinNetworkChannelImports {
        self.channel
    }

    /// Returns the unique function slot assigned to a closed adapter role.
    ///
    /// # Panics
    ///
    /// Only if this privately constructed value violates its complete-catalog invariant.
    #[must_use]
    pub fn function(&self, role: DarwinNetworkAdapterFunction) -> Arm64FunctionImportId {
        self.functions
            .iter()
            .find_map(|(candidate, target)| (*candidate == role).then_some(*target))
            .expect("complete Darwin network function catalog")
    }

    /// Returns the unique data slot assigned to a closed adapter role.
    ///
    /// # Panics
    ///
    /// Only if this privately constructed value violates its complete-catalog invariant.
    #[must_use]
    pub fn data(&self, role: DarwinNetworkAdapterData) -> Arm64DataImportId {
        self.data
            .iter()
            .find_map(|(candidate, target)| (*candidate == role).then_some(*target))
            .expect("complete Darwin network data catalog")
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::{
        DarwinNetworkAdapterData, DarwinNetworkAdapterFunction, RuntimeImport,
    };

    use super::Arm64DarwinNetworkAdapterImports;
    use crate::{Arm64CodeBuilder, Arm64Instruction, Arm64ProgramBuilder, Arm64Register};

    #[test]
    fn adapter_imports_are_complete_and_deduplicated() {
        let mut program = Arm64ProgramBuilder::new();
        let first = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        let second = Arm64DarwinNetworkAdapterImports::declare(&mut program).unwrap();
        assert_eq!(first, second);
        for role in DarwinNetworkAdapterFunction::ALL {
            assert_eq!(first.function(*role), second.function(*role));
        }
        for role in DarwinNetworkAdapterData::ALL {
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
        let expected = DarwinNetworkAdapterFunction::ALL
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
        assert_eq!(
            program
                .runtime_imports()
                .iter()
                .map(|entry| entry.import().clone())
                .collect::<Vec<_>>(),
            expected
        );
    }
}
