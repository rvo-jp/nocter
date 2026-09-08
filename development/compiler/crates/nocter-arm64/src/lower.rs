use std::fmt;

use nocter_machine::{MachineFunctionId, MachineProgramRoot, MachineTestId};

use crate::{
    Arm64AsyncCancelError, Arm64AsyncConstructorError, Arm64AsyncConsumeError,
    Arm64AsyncFunctionPlan, Arm64AsyncFunctionPlanError, Arm64AsyncResumeError,
    Arm64FunctionTargets, Arm64FunctionTargetsError, Arm64MaterializationError, Arm64Program,
    Arm64ProgramBuilder, Arm64ProgramError, Arm64SelectedFunction, Arm64SelectionError,
};

type LoweredProgram = (Arm64Program, Arm64FunctionTargets);

enum SelectedMachineFunction {
    Immediate(Box<Arm64SelectedFunction>),
    Deferred(Box<Arm64AsyncFunctionPlan>),
}

#[derive(Clone, Copy)]
struct LoweringResources<'a> {
    functions: &'a Arm64FunctionTargets,
    async_primitives: &'a crate::Arm64AsyncPrimitiveTargets,
    data: &'a [(nocter_machine::MachineDataId, crate::Arm64DataId)],
    imports: &'a [(nocter_machine::MachineImportId, crate::Arm64DataId)],
    pack_callbacks: &'a [(crate::Arm64PackCallbackKey, crate::Arm64FunctionId)],
    allocation_failure_error: crate::Arm64DataId,
}

impl SelectedMachineFunction {
    const fn owner(&self) -> MachineFunctionId {
        match self {
            Self::Immediate(function) => function.owner(),
            Self::Deferred(function) => function.owner(),
        }
    }

    const fn body(&self) -> &Arm64SelectedFunction {
        match self {
            Self::Immediate(function) => function,
            Self::Deferred(function) => function.selected(),
        }
    }
}

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
    let selected = select_machine_functions(machine)?;
    let mut builder = Arm64ProgramBuilder::new();
    let functions = Arm64FunctionTargets::declare(machine, &mut builder)?;
    let async_primitives = crate::Arm64AsyncPrimitiveTargets::declare(machine, &mut builder);
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
    let imports = machine
        .imports()
        .map(|(source, descriptor)| {
            Ok((
                source,
                builder.add_function_import(descriptor.import().clone())?,
            ))
        })
        .collect::<Result<Vec<_>, Arm64ProgramError>>()?;
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
    let resources = LoweringResources {
        functions: &functions,
        async_primitives: &async_primitives,
        data: &data,
        imports: &imports,
        pack_callbacks: &pack_callbacks,
        allocation_failure_error,
    };
    define_machine_functions(&selected, resources, &mut builder)?;
    define_async_primitives(async_primitives, &mut builder)?;
    for (key, target) in &pack_callbacks {
        let function = selected
            .get(key.owner().index())
            .filter(|function| function.owner() == key.owner())
            .ok_or(Arm64LoweringError::UnknownFunction(key.owner()))?;
        builder.define_function(
            *target,
            crate::pack_callback::materialize(machine, function.body(), *key, &functions)?,
        )?;
    }
    let entry = functions
        .get(root)
        .map(crate::Arm64FunctionTarget::callable)
        .ok_or(Arm64LoweringError::UnknownFunction(root))?;
    builder.set_entry(entry)?;
    let program = builder.finish().map_err(Arm64LoweringError::Program)?;
    Ok((program, functions))
}

fn select_machine_functions(
    machine: &nocter_machine::MachineProgram,
) -> Result<Vec<SelectedMachineFunction>, Arm64LoweringError> {
    machine
        .functions()
        .map(|(id, function)| match function.execution() {
            nocter_machine::MachineFunctionExecution::Immediate => {
                Arm64SelectedFunction::build(machine, id)
                    .map(Box::new)
                    .map(SelectedMachineFunction::Immediate)
                    .map_err(Arm64LoweringError::from)
            }
            nocter_machine::MachineFunctionExecution::Deferred(_) => {
                Arm64AsyncFunctionPlan::build(machine, id)
                    .map(Box::new)
                    .map(SelectedMachineFunction::Deferred)
                    .map_err(Arm64LoweringError::from)
            }
        })
        .collect()
}

fn define_machine_functions(
    selected: &[SelectedMachineFunction],
    resources: LoweringResources<'_>,
    builder: &mut Arm64ProgramBuilder,
) -> Result<(), Arm64LoweringError> {
    for function in selected {
        let target = resources
            .functions
            .get(function.owner())
            .ok_or(Arm64LoweringError::UnknownFunction(function.owner()))?;
        match function {
            SelectedMachineFunction::Immediate(function) => builder.define_function(
                target.callable(),
                function.materialize(
                    resources.functions,
                    resources.async_primitives,
                    resources.data,
                    resources.imports,
                    resources.pack_callbacks,
                    resources.allocation_failure_error,
                )?,
            )?,
            SelectedMachineFunction::Deferred(function) => {
                define_deferred_function(function, target, resources, builder)?;
            }
        }
    }
    Ok(())
}

fn define_deferred_function(
    function: &Arm64AsyncFunctionPlan,
    target: crate::Arm64FunctionTarget,
    resources: LoweringResources<'_>,
    builder: &mut Arm64ProgramBuilder,
) -> Result<(), Arm64LoweringError> {
    let lifecycle = target
        .asynchronous()
        .ok_or(Arm64LoweringError::UnknownFunction(function.owner()))?;
    builder.define_function(target.callable(), function.materialize_constructor(target)?)?;
    builder.define_function(
        lifecycle.resume(),
        function.materialize_resume(
            target,
            resources.functions,
            resources.async_primitives,
            resources.data,
            resources.imports,
            resources.pack_callbacks,
            resources.allocation_failure_error,
        )?,
    )?;
    builder.define_function(
        lifecycle.cancel(),
        function.materialize_cancel(target, resources.functions)?,
    )?;
    builder.define_function(lifecycle.consume(), function.materialize_consume(target)?)?;
    Ok(())
}

fn define_async_primitives(
    targets: crate::Arm64AsyncPrimitiveTargets,
    builder: &mut Arm64ProgramBuilder,
) -> Result<(), Arm64LoweringError> {
    if let Some(lifecycle) = targets.single_interest_lifecycle() {
        if let Some(constructor) = targets.descriptor_readiness() {
            builder.define_function(
                constructor,
                crate::async_interest_code::materialize_descriptor_constructor(lifecycle)
                    .map_err(Arm64MaterializationError::Code)?,
            )?;
        }
        if let Some(constructor) = targets.monotonic_deadline() {
            builder.define_function(
                constructor,
                crate::async_interest_code::materialize_deadline_constructor(lifecycle)
                    .map_err(Arm64MaterializationError::Code)?,
            )?;
        }
        define_async_interest_lifecycle(lifecycle, builder)?;
    }
    if let (Some(constructor), Some(lifecycle)) = (
        targets.descriptor_readiness_or_deadline(),
        targets.dual_interest_lifecycle(),
    ) {
        builder.define_function(
            constructor,
            crate::async_interest_code::materialize_descriptor_or_deadline_constructor(lifecycle)
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        define_async_interest_lifecycle(lifecycle, builder)?;
    }
    if let Some(join) = targets.task_join() {
        builder.define_function(
            join.constructor(),
            crate::async_join_code::materialize_constructor(join)
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        builder.define_function(
            join.resume(),
            crate::async_join_code::materialize_resume()
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        builder.define_function(
            join.cancel(),
            crate::async_join_code::materialize_cancel()
                .map_err(Arm64MaterializationError::Code)?,
        )?;
        builder.define_function(
            join.consume(),
            crate::async_join_code::materialize_consume()
                .map_err(Arm64MaterializationError::Code)?,
        )?;
    }
    Ok(())
}

fn define_async_interest_lifecycle(
    lifecycle: crate::Arm64AsyncInterestLifecycleTargets,
    builder: &mut Arm64ProgramBuilder,
) -> Result<(), Arm64LoweringError> {
    builder.define_function(
        lifecycle.resume(),
        crate::async_interest_code::materialize_resume(lifecycle.interest_count())
            .map_err(Arm64MaterializationError::Code)?,
    )?;
    builder.define_function(
        lifecycle.cancel(),
        crate::async_interest_code::materialize_cancel(lifecycle.interest_count())
            .map_err(Arm64MaterializationError::Code)?,
    )?;
    builder.define_function(
        lifecycle.consume(),
        crate::async_interest_code::materialize_consume(lifecycle.interest_count())
            .map_err(Arm64MaterializationError::Code)?,
    )?;
    Ok(())
}

fn function_target(
    functions: &Arm64FunctionTargets,
    source: MachineFunctionId,
) -> Result<crate::Arm64FunctionId, Arm64LoweringError> {
    functions
        .get(source)
        .map(crate::Arm64FunctionTarget::callable)
        .ok_or(Arm64LoweringError::UnknownFunction(source))
}

#[derive(Debug)]
pub enum Arm64LoweringError {
    ExpectedProcessProgram,
    ExpectedTestProgram,
    NonDenseData(nocter_machine::MachineDataId),
    UnknownFunction(MachineFunctionId),
    Selection(Arm64SelectionError),
    AsyncPlan(Arm64AsyncFunctionPlanError),
    AsyncConstructor(Arm64AsyncConstructorError),
    AsyncResume(Arm64AsyncResumeError),
    AsyncCancel(Arm64AsyncCancelError),
    AsyncConsume(Arm64AsyncConsumeError),
    FunctionTargets(Arm64FunctionTargetsError),
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
            Self::AsyncPlan(error) => Some(error),
            Self::AsyncConstructor(error) => Some(error),
            Self::AsyncResume(error) => Some(error),
            Self::AsyncCancel(error) => Some(error),
            Self::AsyncConsume(error) => Some(error),
            Self::FunctionTargets(error) => Some(error),
            Self::Materialization(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ExpectedProcessProgram
            | Self::ExpectedTestProgram
            | Self::NonDenseData(_)
            | Self::UnknownFunction(_) => None,
        }
    }
}

impl From<Arm64FunctionTargetsError> for Arm64LoweringError {
    fn from(error: Arm64FunctionTargetsError) -> Self {
        Self::FunctionTargets(error)
    }
}

impl From<Arm64SelectionError> for Arm64LoweringError {
    fn from(error: Arm64SelectionError) -> Self {
        Self::Selection(error)
    }
}

impl From<Arm64AsyncFunctionPlanError> for Arm64LoweringError {
    fn from(error: Arm64AsyncFunctionPlanError) -> Self {
        Self::AsyncPlan(error)
    }
}

impl From<Arm64AsyncConstructorError> for Arm64LoweringError {
    fn from(error: Arm64AsyncConstructorError) -> Self {
        Self::AsyncConstructor(error)
    }
}

impl From<Arm64AsyncResumeError> for Arm64LoweringError {
    fn from(error: Arm64AsyncResumeError) -> Self {
        Self::AsyncResume(error)
    }
}

impl From<Arm64AsyncCancelError> for Arm64LoweringError {
    fn from(error: Arm64AsyncCancelError) -> Self {
        Self::AsyncCancel(error)
    }
}

impl From<Arm64AsyncConsumeError> for Arm64LoweringError {
    fn from(error: Arm64AsyncConsumeError) -> Self {
        Self::AsyncConsume(error)
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
