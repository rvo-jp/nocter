use std::fmt;

use nocter_machine::{MachineFunctionId, MachineProgramRoot};

use crate::{
    Arm64MaterializationError, Arm64Program, Arm64ProgramBuilder, Arm64ProgramError,
    Arm64SelectedFunction, Arm64SelectionError,
};

impl Arm64Program {
    /// Selects and materializes a complete process machine program.
    ///
    /// # Errors
    ///
    /// Rejects test-root programs at the single-entry executable boundary, malformed dense
    /// identities, unsupported selected operations, materialization failures, or final program
    /// layout failures.
    pub fn lower_machine(
        machine: &nocter_machine::MachineProgram,
    ) -> Result<Self, Arm64LoweringError> {
        let MachineProgramRoot::Process { root, .. } = *machine.root() else {
            return Err(Arm64LoweringError::TestProgram);
        };
        let selected = machine
            .functions()
            .map(|(id, _)| Arm64SelectedFunction::build(machine, id))
            .collect::<Result<Vec<_>, _>>()?;
        let mut builder = Arm64ProgramBuilder::new();
        let mut functions = Vec::with_capacity(selected.len());
        for function in &selected {
            if function.owner().index() != functions.len() {
                return Err(Arm64LoweringError::NonDenseFunction(function.owner()));
            }
            functions.push((function.owner(), builder.declare_function()));
        }
        let mut pack_callbacks = Vec::new();
        for function in &selected {
            let body = machine
                .function(function.owner())
                .ok_or(Arm64LoweringError::UnknownFunction(function.owner()))?
                .body();
            for (pack, _) in body.packs() {
                for kind in [
                    crate::Arm64PackCallbackKind::Next,
                    crate::Arm64PackCallbackKind::Destroy,
                ] {
                    pack_callbacks.push((
                        crate::Arm64PackCallbackKey::new(function.owner(), pack, kind),
                        builder.declare_function(),
                    ));
                }
            }
        }
        let mut data = Vec::with_capacity(machine.data().len());
        for (source, definition) in machine.data().iter() {
            if source.index() != data.len() {
                return Err(Arm64LoweringError::NonDenseData(source));
            }
            data.push((source, builder.add_data(definition.bytes(), 1)?));
        }
        for function in &selected {
            let target = functions
                .get(function.owner().index())
                .and_then(|(owner, target)| (*owner == function.owner()).then_some(*target))
                .ok_or(Arm64LoweringError::UnknownFunction(function.owner()))?;
            builder.define_function(
                target,
                function.materialize(&functions, &data, &pack_callbacks)?,
            )?;
        }
        for (key, target) in &pack_callbacks {
            let function = selected
                .get(key.owner().index())
                .filter(|function| function.owner() == key.owner())
                .ok_or(Arm64LoweringError::UnknownFunction(key.owner()))?;
            builder.define_function(
                *target,
                crate::pack_callback::materialize(machine, function, *key, &functions)?,
            )?;
        }
        let entry = functions
            .get(root.index())
            .and_then(|(owner, target)| (*owner == root).then_some(*target))
            .ok_or(Arm64LoweringError::UnknownFunction(root))?;
        builder.set_entry(entry)?;
        builder.finish().map_err(Arm64LoweringError::Program)
    }
}

#[derive(Debug)]
pub enum Arm64LoweringError {
    TestProgram,
    NonDenseFunction(MachineFunctionId),
    NonDenseData(nocter_machine::MachineDataId),
    UnknownFunction(MachineFunctionId),
    Selection(Arm64SelectionError),
    Materialization(Arm64MaterializationError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64LoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 program lowering failed: {self:?}")
    }
}

impl std::error::Error for Arm64LoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Selection(error) => Some(error),
            Self::Materialization(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::TestProgram
            | Self::NonDenseFunction(_)
            | Self::NonDenseData(_)
            | Self::UnknownFunction(_) => None,
        }
    }
}

impl From<Arm64SelectionError> for Arm64LoweringError {
    fn from(error: Arm64SelectionError) -> Self {
        Self::Selection(error)
    }
}

impl From<Arm64MaterializationError> for Arm64LoweringError {
    fn from(error: Arm64MaterializationError) -> Self {
        Self::Materialization(error)
    }
}

impl From<Arm64ProgramError> for Arm64LoweringError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}
