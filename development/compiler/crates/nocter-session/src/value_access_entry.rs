//! Selects a package entry from one sealed v2 analysis and its exact discovery input.
use std::{fmt, sync::Arc};

use nocter_checking::value_access::accepted::Unit;
use nocter_compile_input::{CompileUnitInput, PackageTargetResolutionInput};
use nocter_declarations::value_access::{DeclarationId, Namespace};
use nocter_diagnostics::DiagnosticCode;
use nocter_model::{AccessCodeId, CompilationTarget, PackageTargetKind};
use nocter_semantic_product::{AcceptedValueAccess, ValueAccessAnalysis};
use nocter_target_program::{
    EXECUTABLE_ENTRY_NAME, EntryContractRule, ProcessResultContract, TargetUnavailable,
    value_access,
};

use crate::{
    AnalyzedUnit, ExecutableIdentity, ExecutableSelectionError, ExecutableSelector,
    executable::select_candidate,
};

/// Checked entry paired with the target declaration and accepted unit that selected it.
#[derive(Debug)]
pub struct ValueAccessExecutable<'a> {
    declaration: &'a PackageTargetResolutionInput,
    target: CompilationTarget,
    accepted: &'a AcceptedValueAccess,
    entry: AccessCodeId,
    result: ProcessResultContract,
}

impl ValueAccessExecutable<'_> {
    #[must_use]
    pub fn identity(&self) -> ExecutableIdentity {
        executable_identity(self.declaration)
    }

    #[must_use]
    pub const fn declaration(&self) -> &PackageTargetResolutionInput {
        self.declaration
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

fn executable_identity(declaration: &PackageTargetResolutionInput) -> ExecutableIdentity {
    ExecutableIdentity::new(declaration.module().package().clone(), declaration.name())
}

#[derive(Debug)]
pub enum ValueAccessEntryError {
    RequiresAcceptedUnit,
    CompileInput(nocter_discovery::CompileInputError),
    Target(TargetUnavailable),
    Selection(ExecutableSelectionError),
    InvalidMain {
        executable: ExecutableIdentity,
        rule: Option<EntryContractRule>,
    },
}

impl ValueAccessEntryError {
    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<DiagnosticCode> {
        match self {
            Self::Selection(_) | Self::InvalidMain { .. } => Some(DiagnosticCode::E0800),
            Self::Target(_) => Some(DiagnosticCode::E0701),
            Self::RequiresAcceptedUnit | Self::CompileInput(_) => None,
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
            Self::Target(error) => error.fmt(formatter),
            Self::Selection(error) => error.fmt(formatter),
            Self::InvalidMain {
                executable,
                rule: None,
            } => {
                write!(
                    formatter,
                    "executable {} in {} requires main in its module's index.nct",
                    executable.name(),
                    executable.package().as_str()
                )
            }
            Self::InvalidMain {
                executable,
                rule: Some(rule),
            } => {
                write!(
                    formatter,
                    "main for executable {} in {} violates its {rule:?} contract",
                    executable.name(),
                    executable.package().as_str()
                )
            }
        }
    }
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

/// Selects an executable declared by a root package and checks its own authored entry contract.
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
    let declaration = select_candidate(root_executables(input), selector, |target| target.name())
        .map_err(ValueAccessEntryError::Selection)?;
    close_entry(accepted, input.target(), declaration)
}

/// Checks every root-package executable in package target declaration order.
///
/// # Errors
/// Rejects a failed or non-v2 analysis, unavailable compile input, or invalid entry contract.
pub fn select_all_value_access_executables(
    analysis: &AnalyzedUnit,
) -> Result<Vec<ValueAccessExecutable<'_>>, ValueAccessEntryError> {
    let (accepted, input) = accepted_input(analysis)?;
    root_executables(input)
        .map(|declaration| close_entry(accepted, input.target(), declaration))
        .collect()
}

/// Qualifies the selected v2 package entries and platform before any native lowering.
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
        for declaration in root_executables(input) {
            close_entry(accepted, input.target(), declaration)?;
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

fn root_executables<'a>(
    input: &'a CompileUnitInput<'_>,
) -> impl Iterator<Item = &'a PackageTargetResolutionInput> {
    input.package_target_resolutions().iter().filter(|target| {
        target.kind() == PackageTargetKind::Executable
            && input.root_packages().contains(target.module().package())
    })
}

fn close_entry<'a>(
    accepted: &'a AcceptedValueAccess,
    target: CompilationTarget,
    declaration: &'a PackageTargetResolutionInput,
) -> Result<ValueAccessExecutable<'a>, ValueAccessEntryError> {
    let module = declaration.module();
    let headers = accepted.headers();
    let invalid_main = |rule| ValueAccessEntryError::InvalidMain {
        executable: executable_identity(declaration),
        rule,
    };
    let entry = headers
        .program()
        .symbols()
        .get(EXECUTABLE_ENTRY_NAME)
        .and_then(|name| headers.lookup_export(module, module, Namespace::Value, name));
    let entry = match entry {
        Some(DeclarationId::Code(entry)) => entry,
        None => return Err(invalid_main(None)),
        Some(_) => return Err(invalid_main(Some(EntryContractRule::TopLevelFunction))),
    };
    let result = value_access::immediate_entry_result(accepted.unit(), entry)
        .map_err(|rule| invalid_main(Some(rule)))?;
    Ok(ValueAccessExecutable {
        declaration,
        target,
        accepted,
        entry,
        result,
    })
}
