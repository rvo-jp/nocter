//! Selects an entry from one sealed v2 analysis and its exact discovery input.
use std::{fmt, sync::Arc};

use nocter_checking::value_access::accepted::Unit;
use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleSourceKind, PackageMode, PackageTargetResolutionInput,
};
use nocter_declarations::value_access::{CodeKind, DeclarationId, Namespace};
use nocter_diagnostics::{DiagnosticCode, DiagnosticOrigin, SourceDiagnostic};
use nocter_model::{AccessCodeId, CompilationTarget, PackageIdentity, PackageTargetKind};
use nocter_semantic_product::{AcceptedValueAccess, ValueAccessAnalysis};
use nocter_source::{ByteOffset, SourceId, Span, TextRange};
use nocter_source_index::SourceOrigin;
use nocter_target_program::{
    EXECUTABLE_ENTRY_NAME, EntryContractRule, ProcessResultContract, TargetUnavailable,
    value_access,
};

use crate::{
    AnalyzedUnit, ExecutableIdentity, ExecutableSelectionError, ExecutableSelector,
    executable::select_candidate,
};

/// Checked entry paired with its exact discovery origin and accepted unit.
#[derive(Debug)]
pub struct ValueAccessExecutable<'a> {
    origin: EntryOrigin<'a>,
    target: CompilationTarget,
    accepted: &'a AcceptedValueAccess,
    entry: AccessCodeId,
    result: ProcessResultContract,
}

impl ValueAccessExecutable<'_> {
    #[must_use]
    pub fn identity(&self) -> ExecutableIdentity {
        self.origin.identity()
    }

    #[must_use]
    pub const fn package_declaration(&self) -> Option<&PackageTargetResolutionInput> {
        match self.origin {
            EntryOrigin::Package(declaration) => Some(declaration),
            EntryOrigin::SingleFile { .. } => None,
        }
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub const fn result(&self) -> ProcessResultContract {
        self.result
    }

    #[must_use]
    pub fn into_entry(self) -> (Arc<Unit>, AccessCodeId) {
        (self.accepted.unit_handle(), self.entry)
    }
}

#[derive(Clone, Copy, Debug)]
enum EntryOrigin<'a> {
    Package(&'a PackageTargetResolutionInput),
    SingleFile {
        module: &'a ModuleIdentity,
        path: &'a str,
        name: &'a str,
        source: SourceId,
    },
}

impl<'a> EntryOrigin<'a> {
    fn name(self) -> &'a str {
        match self {
            Self::Package(declaration) => declaration.name(),
            Self::SingleFile { name, .. } => name,
        }
    }

    fn identity(self) -> ExecutableIdentity {
        let module = match self {
            Self::Package(declaration) => declaration.module(),
            Self::SingleFile { module, .. } => module,
        };
        ExecutableIdentity::new(module.package().clone(), self.name())
    }

    fn diagnostic_origin(self, input: &CompileUnitInput<'_>) -> Option<DiagnosticOrigin> {
        match self {
            Self::Package(declaration) => {
                let name = declaration.name_literal();
                let tree = input.syntax_tree(name.source())?;
                SourceOrigin::from_node(tree, name).ok().map(Into::into)
            }
            Self::SingleFile { source, .. } => {
                input.sources().get(source)?;
                Some(Span::new(source, TextRange::empty(ByteOffset::new(0))).into())
            }
        }
    }
}

#[derive(Debug)]
pub enum ValueAccessEntryError {
    RequiresAcceptedUnit,
    CompileInput(nocter_discovery::CompileInputError),
    InvalidSingleFileInput,
    Target(TargetUnavailable),
    Selection(ExecutableSelectionError),
    InvalidMain {
        executable: ExecutableIdentity,
        rule: Option<EntryContractRule>,
        single_file: bool,
        diagnostic: Option<Box<SourceDiagnostic>>,
    },
}

impl ValueAccessEntryError {
    #[must_use]
    pub fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        match self {
            Self::InvalidMain {
                diagnostic: Some(diagnostic),
                ..
            } => std::slice::from_ref(diagnostic),
            _ => &[],
        }
    }

    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<DiagnosticCode> {
        match self {
            Self::Selection(_) | Self::InvalidMain { .. } => Some(DiagnosticCode::E0800),
            Self::Target(_) => Some(DiagnosticCode::E0701),
            Self::RequiresAcceptedUnit | Self::CompileInput(_) | Self::InvalidSingleFileInput => {
                None
            }
        }
    }
}

impl fmt::Display for ValueAccessEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiresAcceptedUnit => {
                formatter.write_str("executable selection requires successful v2 analysis")
            }
            Self::CompileInput(error) => error.fmt(formatter),
            Self::InvalidSingleFileInput => {
                formatter.write_str("v2 standalone input does not identify one entry file")
            }
            Self::Target(error) => error.fmt(formatter),
            Self::Selection(error) => error.fmt(formatter),
            Self::InvalidMain {
                diagnostic: Some(diagnostic),
                ..
            } => formatter.write_str(diagnostic.message()),
            Self::InvalidMain {
                executable,
                rule,
                single_file,
                diagnostic: None,
            } => formatter.write_str(&invalid_main_message(executable, *rule, *single_file)),
        }
    }
}

fn invalid_main_message(
    executable: &ExecutableIdentity,
    rule: Option<EntryContractRule>,
    single_file: bool,
) -> String {
    let location = if single_file {
        "the selected source file"
    } else {
        "its module's index.nct"
    };
    let requirement = match rule {
        None => format!("requires main in {location}"),
        Some(EntryContractRule::TopLevelFunction) => {
            "requires main to be a top-level function".into()
        }
        Some(EntryContractRule::GenericParameters) => {
            "requires main to have no generic parameters".into()
        }
        Some(EntryContractRule::ValueParameters) => {
            "requires main to have no value parameters".into()
        }
        Some(EntryContractRule::MissingBody) => "requires a checked body for main".into(),
        Some(EntryContractRule::ResultType) => {
            "requires main to return void, i32, usize, or a fallible form of one of those types"
                .into()
        }
    };
    format!("executable {} {requirement}", executable.name())
}

impl std::error::Error for ValueAccessEntryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CompileInput(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::Selection(error) => Some(error),
            _ => None,
        }
    }
}

/// Selects a root package or explicit-file executable and checks its authored entry.
///
/// # Errors
/// Rejects a failed or non-v2 analysis, unavailable compile input, ambiguous target, or invalid
/// entry contract.
pub fn select_value_access_executable<'a>(
    analysis: &'a AnalyzedUnit,
    selector: &ExecutableSelector,
) -> Result<ValueAccessExecutable<'a>, ValueAccessEntryError> {
    let (accepted, input) = accepted_input(analysis)?;
    select_entry(accepted, input, selector)
}

fn select_entry<'a>(
    accepted: &'a AcceptedValueAccess,
    input: &'a CompileUnitInput<'static>,
    selector: &ExecutableSelector,
) -> Result<ValueAccessExecutable<'a>, ValueAccessEntryError> {
    let origin = select_candidate(entry_origins(input)?, selector, |origin| origin.name())
        .map_err(ValueAccessEntryError::Selection)?;
    close_entry(accepted, input, origin)
}

/// Checks every selected executable in package target declaration order.
///
/// # Errors
/// Rejects a failed or non-v2 analysis, unavailable compile input, or invalid entry contract.
pub fn select_all_value_access_executables(
    analysis: &AnalyzedUnit,
) -> Result<Vec<ValueAccessExecutable<'_>>, ValueAccessEntryError> {
    let (accepted, input) = accepted_input(analysis)?;
    entry_origins(input)?
        .map(|origin| close_entry(accepted, input, origin))
        .collect()
}

/// Qualifies the selected v2 entries and platform before any native lowering.
/// A library package has no entry to qualify.
///
/// # Errors
/// Rejects source domains without an accepted v2 unit, invalid entry contracts or unavailable targets.
pub fn check_value_access_target(
    analysis: &AnalyzedUnit,
    selector: Option<&ExecutableSelector>,
) -> Result<(), ValueAccessEntryError> {
    let (accepted, input) = accepted_input(analysis)?;
    if let Some(selector) = selector {
        select_entry(accepted, input, selector)?;
    } else {
        for origin in entry_origins(input)? {
            close_entry(accepted, input, origin)?;
        }
    }
    value_access::executable_abi(input.target()).map_err(ValueAccessEntryError::Target)?;
    Ok(())
}

fn accepted_input(
    analysis: &AnalyzedUnit,
) -> Result<(&AcceptedValueAccess, &CompileUnitInput<'static>), ValueAccessEntryError> {
    let Some(ValueAccessAnalysis::Accepted(accepted)) = analysis.value_access_analysis() else {
        return Err(ValueAccessEntryError::RequiresAcceptedUnit);
    };
    let input = analysis
        .unit()
        .compile_input()
        .map_err(ValueAccessEntryError::CompileInput)?;
    Ok((accepted, input))
}

/// Cases selected from one accepted package target and its authored test identities.
pub struct ValueAccessTests {
    target: CompilationTarget,
    unit: Arc<Unit>,
    cases: Box<[(AccessCodeId, Box<str>)]>,
}

impl ValueAccessTests {
    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub fn unit(&self) -> Arc<Unit> {
        Arc::clone(&self.unit)
    }

    #[must_use]
    pub fn cases(&self) -> impl ExactSizeIterator<Item = (AccessCodeId, &str)> {
        self.cases.iter().map(|(code, name)| (*code, name.as_ref()))
    }
}

#[derive(Debug)]
pub enum ValueAccessTestError {
    Source(ValueAccessEntryError),
    InvalidTarget,
    MissingCase,
    InconsistentCase,
}

impl fmt::Display for ValueAccessTestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::InvalidTarget => {
                formatter.write_str("test target does not belong to the selected package")
            }
            Self::MissingCase => formatter.write_str("selected test target has no matching case"),
            Self::InconsistentCase => {
                formatter.write_str("selected test case has no checked declaration")
            }
        }
    }
}

impl std::error::Error for ValueAccessTestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            _ => None,
        }
    }
}

/// Selects tests directly authored in the named target module of the exact root package.
/// # Errors
/// Rejects unaccepted units, foreign or missing targets, and absent or inconsistent case identities.
pub fn select_value_access_tests(
    analysis: &AnalyzedUnit,
    package: &PackageIdentity,
    target_name: &str,
    case: Option<&str>,
) -> Result<ValueAccessTests, ValueAccessTestError> {
    let (accepted, input) = accepted_input(analysis).map_err(ValueAccessTestError::Source)?;
    let mut selected = input.package_target_resolutions().iter().filter(|target| {
        target.kind() == PackageTargetKind::Test
            && target.module().package() == package
            && input.root_packages().contains(package)
            && target.name() == target_name
    });
    let target = selected.next().ok_or(ValueAccessTestError::InvalidTarget)?;
    if selected.next().is_some() {
        return Err(ValueAccessTestError::InvalidTarget);
    }

    let headers = accepted.headers();
    let program = headers.program();
    let mut cases = Vec::new();
    for id in headers.test_codes(target.module()) {
        let declaration = program
            .codes()
            .get(id)
            .ok_or(ValueAccessTestError::InconsistentCase)?;
        let CodeKind::Test(name) = declaration.kind() else {
            return Err(ValueAccessTestError::InconsistentCase);
        };
        let name = program
            .symbols()
            .spelling(name)
            .ok_or(ValueAccessTestError::InconsistentCase)?;
        if accepted.unit().body(id).is_none() {
            return Err(ValueAccessTestError::InconsistentCase);
        }
        if case.is_none_or(|case| case == name) {
            cases.push((id, name.into()));
        }
    }
    if case.is_some() && cases.is_empty() {
        return Err(ValueAccessTestError::MissingCase);
    }
    Ok(ValueAccessTests {
        target: input.target(),
        unit: accepted.unit_handle(),
        cases: cases.into_boxed_slice(),
    })
}

fn entry_origins<'a>(
    input: &'a CompileUnitInput<'static>,
) -> Result<impl Iterator<Item = EntryOrigin<'a>>, ValueAccessEntryError> {
    let package_targets = input
        .package_target_resolutions()
        .iter()
        .filter(|target| {
            target.kind() == PackageTargetKind::Executable
                && input.root_packages().contains(target.module().package())
        })
        .map(EntryOrigin::Package);
    Ok(single_file(input)?.into_iter().chain(package_targets))
}

fn single_file<'a>(
    input: &'a CompileUnitInput<'static>,
) -> Result<Option<EntryOrigin<'a>>, ValueAccessEntryError> {
    let Some(package) = input.packages().iter().find(|package| {
        package.mode() == PackageMode::SingleFile
            && input.root_packages().contains(package.identity())
    }) else {
        return Ok(None);
    };
    if input.root_packages().len() != 1 {
        return Err(ValueAccessEntryError::InvalidSingleFileInput);
    }
    let mut sources = input
        .modules()
        .iter()
        .filter(|module| {
            module.identity().package() == package.identity() && module.identity().path().is_empty()
        })
        .flat_map(|module| {
            module
                .sources()
                .iter()
                .filter(|source| source.kind() == ModuleSourceKind::SingleFile)
                .map(move |source| (module.identity(), source))
        });
    let (module, source) = sources
        .next()
        .ok_or(ValueAccessEntryError::InvalidSingleFileInput)?;
    if sources.next().is_some() {
        return Err(ValueAccessEntryError::InvalidSingleFileInput);
    }
    Ok(Some(EntryOrigin::SingleFile {
        module,
        path: source.canonical_path(),
        name: package.display_name(),
        source: source.syntax().source(),
    }))
}

fn close_entry<'a>(
    accepted: &'a AcceptedValueAccess,
    input: &'a CompileUnitInput<'static>,
    origin: EntryOrigin<'a>,
) -> Result<ValueAccessExecutable<'a>, ValueAccessEntryError> {
    let headers = accepted.headers();
    let invalid_main = |rule| {
        let executable = origin.identity();
        let single_file = matches!(origin, EntryOrigin::SingleFile { .. });
        let diagnostic = origin.diagnostic_origin(input).map(|primary| {
            Box::new(SourceDiagnostic::new(
                DiagnosticCode::E0800,
                invalid_main_message(&executable, rule, single_file),
                primary,
                [],
                None::<&str>,
            ))
        });
        ValueAccessEntryError::InvalidMain {
            executable,
            rule,
            single_file,
            diagnostic,
        }
    };
    let entry = headers
        .program()
        .symbols()
        .get(EXECUTABLE_ENTRY_NAME)
        .and_then(|name| match origin {
            EntryOrigin::Package(declaration) => {
                let module = declaration.module();
                headers.lookup_export(module, module, Namespace::Value, name)
            }
            EntryOrigin::SingleFile { module, path, .. } => {
                headers.lookup_single_file_declaration(module, path, Namespace::Value, name)
            }
        });
    let entry = match entry {
        Some(DeclarationId::Code(entry)) => entry,
        None => return Err(invalid_main(None)),
        Some(_) => return Err(invalid_main(Some(EntryContractRule::TopLevelFunction))),
    };
    let result = value_access::entry_result(accepted.unit(), entry)
        .map_err(|rule| invalid_main(Some(rule)))?;
    Ok(ValueAccessExecutable {
        origin,
        target: input.target(),
        accepted,
        entry,
        result,
    })
}
