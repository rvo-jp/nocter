use nocter_runtime_contract::DarwinFileServiceFunction;

use crate::{Arm64FunctionImportId, Arm64ProgramBuilder, Arm64ProgramError};

/// Complete typed loader boundary for the generated Darwin file service.
///
/// Runtime-contract roles own library and symbol selection. This ARM64 projection assigns each
/// role one deduplicated loader slot and is the only interface later file-service emitters receive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64DarwinFileServiceImports {
    functions: Box<[(DarwinFileServiceFunction, Arm64FunctionImportId)]>,
}

impl Arm64DarwinFileServiceImports {
    /// Declares the complete catalog in canonical role order.
    ///
    /// # Errors
    ///
    /// Propagates an ARM64 program data or alignment failure.
    pub fn declare(program: &mut Arm64ProgramBuilder) -> Result<Self, Arm64ProgramError> {
        let functions = DarwinFileServiceFunction::ALL
            .iter()
            .copied()
            .map(|role| Ok((role, program.add_function_import(role.import())?)))
            .collect::<Result<Vec<_>, Arm64ProgramError>>()?
            .into_boxed_slice();
        Ok(Self { functions })
    }

    /// Returns the unique function slot assigned to one closed file-service role.
    ///
    /// # Panics
    ///
    /// Only if this privately constructed value violates its complete-catalog invariant.
    #[must_use]
    pub fn function(&self, role: DarwinFileServiceFunction) -> Arm64FunctionImportId {
        self.functions
            .iter()
            .find_map(|(candidate, target)| (*candidate == role).then_some(*target))
            .expect("complete Darwin file service function catalog")
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::{DarwinFileServiceFunction, RuntimeImport};

    use super::Arm64DarwinFileServiceImports;
    use crate::{Arm64CodeBuilder, Arm64Instruction, Arm64ProgramBuilder, Arm64Register};

    #[test]
    fn file_service_imports_are_complete_and_deduplicated() {
        let mut program = Arm64ProgramBuilder::new();
        let first = Arm64DarwinFileServiceImports::declare(&mut program).unwrap();
        let second = Arm64DarwinFileServiceImports::declare(&mut program).unwrap();
        assert_eq!(first, second);
        for role in DarwinFileServiceFunction::ALL {
            assert_eq!(first.function(*role), second.function(*role));
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
        let expected = DarwinFileServiceFunction::ALL
            .iter()
            .copied()
            .map(|role| RuntimeImport::from(role.import()))
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
