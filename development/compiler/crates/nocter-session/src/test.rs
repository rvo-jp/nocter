use std::fmt;
use std::sync::Arc;

use nocter_arm64::Arm64TestSuite;
use nocter_diagnostics::SourceDiagnostic;
use nocter_discovery::DiscoveredUnit;
use nocter_machine::MachineProgram;
use nocter_macho::MachOImage;
use nocter_mir::lower_executable;
use nocter_model::PackageTargetKind;
use nocter_model::{PackageIdentity, PackageTargetId, TestId};
use nocter_source_index::SourceIndex;
use nocter_target_program::{
    ExecutableProgram, TargetProgram, select_test_case, select_test_target,
};

use crate::{CompileSessionError, NativeImageError, compile_target};

/// Resolver-stable identity of one package-declared test target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestTargetIdentity {
    package: PackageIdentity,
    target: PackageTargetId,
    name: Box<str>,
}

impl TestTargetIdentity {
    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub const fn target(&self) -> PackageTargetId {
        self.target
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }
}

/// Semantic identity of one source-declared test case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestCaseIdentity {
    declaration: TestId,
    name: Box<str>,
}

impl TestCaseIdentity {
    #[must_use]
    pub const fn declaration(&self) -> TestId {
        self.declaration
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }
}

/// User-visible test-target selection, kept separate from source declaration identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TestTargetSelector {
    All,
    Named(Box<str>),
}

impl TestTargetSelector {
    #[must_use]
    pub fn named(name: impl Into<Box<str>>) -> Self {
        Self::Named(name.into())
    }
}

/// One closed native test compilation request.
#[derive(Debug)]
pub struct NativeTestCompileRequest<'unit> {
    unit: &'unit DiscoveredUnit,
    target: TestTargetSelector,
    case: Option<Box<str>>,
}

impl<'unit> NativeTestCompileRequest<'unit> {
    #[must_use]
    pub const fn new(
        unit: &'unit DiscoveredUnit,
        target: TestTargetSelector,
        case: Option<Box<str>>,
    ) -> Self {
        Self { unit, target, case }
    }

    #[must_use]
    pub const fn all(unit: &'unit DiscoveredUnit) -> Self {
        Self::new(unit, TestTargetSelector::All, None)
    }

    #[must_use]
    pub fn named(unit: &'unit DiscoveredUnit, target: impl Into<Box<str>>) -> Self {
        Self::new(unit, TestTargetSelector::named(target), None)
    }

    #[must_use]
    pub fn case(
        unit: &'unit DiscoveredUnit,
        target: impl Into<Box<str>>,
        case: impl Into<Box<str>>,
    ) -> Self {
        Self::new(unit, TestTargetSelector::named(target), Some(case.into()))
    }
}

/// One complete native image paired with its semantic case identity.
#[derive(Debug)]
pub struct NativeTestImage {
    identity: TestCaseIdentity,
    image: MachOImage,
}

impl NativeTestImage {
    #[must_use]
    pub const fn identity(&self) -> &TestCaseIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn image(&self) -> &MachOImage {
        &self.image
    }

    #[must_use]
    pub fn into_parts(self) -> (TestCaseIdentity, MachOImage) {
        (self.identity, self.image)
    }
}

/// Native compilation result for one selected target. A target-wide backend failure is retained
/// beside successful siblings instead of terminating the test-set compilation.
#[derive(Debug)]
pub enum NativeTestTargetOutcome {
    Compiled(Box<[NativeTestImage]>),
    CompileFailed(NativeImageError),
}

/// One declaration-order selected test target and its complete compilation outcome.
#[derive(Debug)]
pub struct NativeTestTargetCompilation {
    identity: TestTargetIdentity,
    outcome: NativeTestTargetOutcome,
}

impl NativeTestTargetCompilation {
    #[must_use]
    pub const fn identity(&self) -> &TestTargetIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn outcome(&self) -> &NativeTestTargetOutcome {
        &self.outcome
    }

    #[must_use]
    pub fn into_parts(self) -> (TestTargetIdentity, NativeTestTargetOutcome) {
        (self.identity, self.outcome)
    }
}

/// Complete ordered native output for a test command's semantic selection.
#[derive(Debug)]
pub struct CompiledNativeTestSet {
    targets: Box<[NativeTestTargetCompilation]>,
    source_index: SourceIndex,
}

impl CompiledNativeTestSet {
    #[must_use]
    pub const fn targets(&self) -> &[NativeTestTargetCompilation] {
        &self.targets
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (Box<[NativeTestTargetCompilation]>, SourceIndex) {
        (self.targets, self.source_index)
    }
}

/// Compiles the selected package test targets through one shared target program.
///
/// Target selection and exact case selection happen over semantic declarations before executable
/// closure. Each target retains an independent backend outcome so one unsupported target does not
/// suppress later targets.
///
/// # Errors
///
/// Returns a shared semantic compilation failure or an invalid target/case selection.
pub fn compile_native_tests(
    request: NativeTestCompileRequest<'_>,
) -> Result<CompiledNativeTestSet, NativeTestSessionError> {
    let NativeTestCompileRequest { unit, target, case } = request;
    let compiled = compile_target(unit)?;
    let (program, source_index) = compiled.into_parts();
    let targets = select_targets(&program, &target)?;
    if case.is_some() && targets.len() != 1 {
        return Err(TestTargetSelectionError::CaseRequiresNamedTarget.into());
    }
    let program = Arc::new(program);
    let mut compiled_targets = Vec::with_capacity(targets.len());
    for identity in targets {
        let mut selected = select_test_target(&program, identity.target()).map_err(|error| {
            NativeTestSessionError::Integrity(
                format!("semantic test selection failed: {error}").into(),
            )
        })?;
        if let Some(case) = case.as_deref() {
            selected = select_test_case(&program, &selected, case).map_err(|error| {
                NativeTestSessionError::Selection(TestTargetSelectionError::Case(error))
            })?;
        }
        let cases = selected
            .tests()
            .iter()
            .map(|test| {
                let name = program
                    .checked()
                    .graph()
                    .symbols()
                    .spelling(test.name())
                    .ok_or_else(|| {
                        NativeTestSessionError::Integrity(
                            "selected test has no validated source name".into(),
                        )
                    })?;
                Ok(TestCaseIdentity {
                    declaration: test.declaration(),
                    name: name.into(),
                })
            })
            .collect::<Result<Vec<_>, NativeTestSessionError>>()?;
        let outcome = compile_test_target(Arc::clone(&program), &selected, cases);
        compiled_targets.push(NativeTestTargetCompilation { identity, outcome });
    }
    Ok(CompiledNativeTestSet {
        targets: compiled_targets.into_boxed_slice(),
        source_index,
    })
}

fn compile_test_target(
    program: Arc<TargetProgram>,
    selected: &nocter_target_program::SelectedTestTarget,
    identities: Vec<TestCaseIdentity>,
) -> NativeTestTargetOutcome {
    let result = (|| {
        let executable = ExecutableProgram::for_selected_tests(program, selected)
            .map_err(NativeImageError::Executable)?;
        let mir = lower_executable(executable).map_err(NativeImageError::Mir)?;
        let machine = MachineProgram::lower(&mir).map_err(NativeImageError::Machine)?;
        let arm64 = Arm64TestSuite::lower_machine(&machine).map_err(NativeImageError::Arm64)?;
        if arm64.tests().len() != identities.len() {
            return Err(NativeImageError::Integrity(
                "native test identities do not match lowered test roots".into(),
            ));
        }
        arm64
            .tests()
            .iter()
            .zip(identities)
            .map(|(test, identity)| {
                if test.name() != identity.name() {
                    return Err(NativeImageError::Integrity(
                        "native test name does not match its semantic identity".into(),
                    ));
                }
                let image = MachOImage::build(test.program()).map_err(NativeImageError::Image)?;
                Ok(NativeTestImage { identity, image })
            })
            .collect::<Result<Vec<_>, NativeImageError>>()
            .map(Vec::into_boxed_slice)
    })();
    match result {
        Ok(images) => NativeTestTargetOutcome::Compiled(images),
        Err(error) => NativeTestTargetOutcome::CompileFailed(error),
    }
}

fn select_targets(
    program: &TargetProgram,
    selector: &TestTargetSelector,
) -> Result<Vec<TestTargetIdentity>, TestTargetSelectionError> {
    let graph = program.checked().graph();
    let candidates = graph
        .package_targets()
        .iter()
        .filter(|(_, target)| {
            target.kind() == PackageTargetKind::Test
                && graph.root_packages().contains(&target.package())
        })
        .map(|(id, target)| {
            let package = graph
                .packages()
                .get(target.package())
                .expect("validated package target retains its package");
            let name = graph
                .symbols()
                .spelling(target.name())
                .expect("validated package target retains its name");
            TestTargetIdentity {
                package: package.identity().clone(),
                target: id,
                name: name.into(),
            }
        })
        .collect::<Vec<_>>();
    match selector {
        TestTargetSelector::All if candidates.is_empty() => Err(TestTargetSelectionError::NoTarget),
        TestTargetSelector::All => Ok(candidates),
        TestTargetSelector::Named(name) => {
            let matching = candidates
                .into_iter()
                .filter(|candidate| candidate.name() == name.as_ref())
                .collect::<Vec<_>>();
            match matching.as_slice() {
                [] => Err(TestTargetSelectionError::UnknownName(name.clone())),
                [_] => Ok(matching),
                _ => Err(TestTargetSelectionError::AmbiguousName(name.clone())),
            }
        }
    }
}

#[derive(Debug)]
pub enum NativeTestSessionError {
    Compile(CompileSessionError),
    Selection(TestTargetSelectionError),
    Integrity(Box<str>),
}

impl NativeTestSessionError {
    #[must_use]
    pub fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Compile(error) => error.source_diagnostic(),
            Self::Selection(_) | Self::Integrity(_) => None,
        }
    }

    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Compile(error) => error.diagnostic_code(),
            Self::Selection(_) => Some("E0800"),
            Self::Integrity(_) => None,
        }
    }
}

impl fmt::Display for NativeTestSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(error) => write!(formatter, "target compilation failed: {error}"),
            Self::Selection(error) => write!(formatter, "test selection failed: {error}"),
            Self::Integrity(message) => {
                write!(formatter, "test compilation integrity failed: {message}")
            }
        }
    }
}

impl std::error::Error for NativeTestSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compile(error) => Some(error),
            Self::Selection(error) => Some(error),
            Self::Integrity(_) => None,
        }
    }
}

impl From<CompileSessionError> for NativeTestSessionError {
    fn from(error: CompileSessionError) -> Self {
        Self::Compile(error)
    }
}

impl From<TestTargetSelectionError> for NativeTestSessionError {
    fn from(error: TestTargetSelectionError) -> Self {
        Self::Selection(error)
    }
}

#[derive(Debug)]
pub enum TestTargetSelectionError {
    NoTarget,
    UnknownName(Box<str>),
    AmbiguousName(Box<str>),
    CaseRequiresNamedTarget,
    Case(nocter_target_program::TestCaseSelectionError),
}

impl fmt::Display for TestTargetSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTarget => formatter.write_str("compile unit declares no test target"),
            Self::UnknownName(name) => {
                write!(formatter, "compile unit has no test target named {name}")
            }
            Self::AmbiguousName(name) => {
                write!(
                    formatter,
                    "test target name {name} is ambiguous across compile roots"
                )
            }
            Self::CaseRequiresNamedTarget => {
                formatter.write_str("an exact test case requires one named test target")
            }
            Self::Case(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TestTargetSelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Case(error) => Some(error),
            Self::NoTarget
            | Self::UnknownName(_)
            | Self::AmbiguousName(_)
            | Self::CaseRequiresNamedTarget => None,
        }
    }
}
