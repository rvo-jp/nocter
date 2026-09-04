use std::fmt;

use nocter_machine::{MachineFunctionId, MachineProgramRoot, MachineTestId};

use crate::{
    Arm64MaterializationError, Arm64Program, Arm64ProgramBuilder, Arm64ProgramError,
    Arm64SelectedFunction, Arm64SelectionError,
};

type LoweredProgram = (
    Arm64Program,
    Box<[(MachineFunctionId, crate::Arm64FunctionId)]>,
);

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
            return Err(Arm64LoweringError::ExpectedProcessProgram);
        };
        lower_machine_entry(machine, root).map(|(program, _)| program)
    }
}

/// One independently launchable native test case with stable presentation metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64TestExecutable {
    id: MachineTestId,
    name: Box<str>,
    program: Arm64Program,
}

impl Arm64TestExecutable {
    #[must_use]
    pub const fn id(&self) -> MachineTestId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn program(&self) -> &Arm64Program {
        &self.program
    }
}

/// Declaration-order native test entries. Every entry shares immutable code and data but receives
/// its own `Arm64Program` entry identity, so a runner can launch each case in a separate process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64TestSuite {
    tests: Box<[Arm64TestExecutable]>,
}

impl Arm64TestSuite {
    /// Lowers every test root without inventing a source `main` or a combined runtime dispatcher.
    ///
    /// # Errors
    ///
    /// Rejects a process program and the same malformed machine or target state as ordinary
    /// process lowering. An empty selected target produces an empty suite.
    pub fn lower_machine(
        machine: &nocter_machine::MachineProgram,
    ) -> Result<Self, Arm64LoweringError> {
        let MachineProgramRoot::Tests(cases) = machine.root() else {
            return Err(Arm64LoweringError::ExpectedTestProgram);
        };
        let Some(first) = cases.first() else {
            return Ok(Self {
                tests: Box::new([]),
            });
        };
        let (base, functions) = lower_machine_entry(machine, first.root())?;
        let tests = cases
            .iter()
            .map(|case| {
                let entry = function_target(&functions, case.root())?;
                Ok(Arm64TestExecutable {
                    id: case.id(),
                    name: case.name().into(),
                    program: base.with_entry(entry)?,
                })
            })
            .collect::<Result<Vec<_>, Arm64LoweringError>>()?
            .into_boxed_slice();
        Ok(Self { tests })
    }

    #[must_use]
    pub const fn tests(&self) -> &[Arm64TestExecutable] {
        &self.tests
    }
}

fn lower_machine_entry(
    machine: &nocter_machine::MachineProgram,
    root: MachineFunctionId,
) -> Result<LoweredProgram, Arm64LoweringError> {
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
    let runtime = machine.layouts().target().runtime_schema();
    let allocation_failure_error = builder.add_data(
        runtime.allocation_failure_error_node(),
        runtime.error().alignment(),
    )?;
    let mut data = Vec::with_capacity(machine.data().len());
    for (source, definition) in machine.data().iter() {
        if source.index() != data.len() {
            return Err(Arm64LoweringError::NonDenseData(source));
        }
        data.push((
            source,
            builder.add_data(definition.bytes(), definition.alignment())?,
        ));
    }
    for (source, definition) in machine.data().iter() {
        let source_target = data
            .get(source.index())
            .and_then(|(actual, target)| (*actual == source).then_some(*target))
            .ok_or(Arm64LoweringError::NonDenseData(source))?;
        for relocation in definition.relocations() {
            let target = data
                .get(relocation.target().index())
                .and_then(|(actual, target)| (*actual == relocation.target()).then_some(*target))
                .ok_or(Arm64LoweringError::NonDenseData(relocation.target()))?;
            builder.add_data_relocation(source_target, relocation.offset(), target)?;
        }
    }
    for function in &selected {
        let target = functions
            .get(function.owner().index())
            .and_then(|(owner, target)| (*owner == function.owner()).then_some(*target))
            .ok_or(Arm64LoweringError::UnknownFunction(function.owner()))?;
        builder.define_function(
            target,
            function.materialize(&functions, &data, &pack_callbacks, allocation_failure_error)?,
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
    let program = builder.finish().map_err(Arm64LoweringError::Program)?;
    Ok((program, functions.into_boxed_slice()))
}

fn function_target(
    functions: &[(MachineFunctionId, crate::Arm64FunctionId)],
    source: MachineFunctionId,
) -> Result<crate::Arm64FunctionId, Arm64LoweringError> {
    functions
        .get(source.index())
        .and_then(|(actual, target)| (*actual == source).then_some(*target))
        .ok_or(Arm64LoweringError::UnknownFunction(source))
}

#[derive(Debug)]
pub enum Arm64LoweringError {
    ExpectedProcessProgram,
    ExpectedTestProgram,
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
            Self::ExpectedProcessProgram
            | Self::ExpectedTestProgram
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
