//! Selects a package entry from one sealed v2 analysis and its exact discovery input.
use std::{fmt, sync::Arc};

use nocter_checking::value_access::accepted::Unit;
use nocter_compile_input::PackageTargetResolutionInput;
use nocter_declarations::value_access::{DeclarationId, Namespace};
use nocter_model::{AccessCodeId, CompilationTarget, PackageTargetKind};
use nocter_semantic_product::{AcceptedValueAccess, ValueAccessAnalysis};
use nocter_target_program::{
    EXECUTABLE_ENTRY_NAME, EntryContractRule, ProcessResultContract, value_access,
};

use crate::{
    AnalyzedUnit, ExecutableSelectionError, ExecutableSelector, executable::select_candidate,
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

#[derive(Debug)]
pub enum ValueAccessEntryError {
    RequiresAcceptedUnit,
    CompileInput(nocter_discovery::CompileInputError),
    Selection(ExecutableSelectionError),
    InvalidMain {
        executable: Box<str>,
        rule: Option<EntryContractRule>,
    },
}

impl fmt::Display for ValueAccessEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiresAcceptedUnit => {
                formatter.write_str("executable selection requires successful v2 analysis")
            }
            Self::CompileInput(error) => error.fmt(formatter),
            Self::Selection(error) => error.fmt(formatter),
            Self::InvalidMain {
                executable,
                rule: None,
            } => {
                write!(
                    formatter,
                    "executable {executable} requires main in its module's index.nct"
                )
            }
            Self::InvalidMain {
                executable,
                rule: Some(rule),
            } => {
                write!(
                    formatter,
                    "main for executable {executable} violates its {rule:?} contract"
                )
            }
        }
    }
}

impl std::error::Error for ValueAccessEntryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CompileInput(error) => Some(error),
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
    let Some(ValueAccessAnalysis::Accepted(accepted)) = analysis.value_access_analysis() else {
        return Err(ValueAccessEntryError::RequiresAcceptedUnit);
    };
    let input = analysis
        .unit()
        .compile_input()
        .map_err(ValueAccessEntryError::CompileInput)?;
    let candidates = input.package_target_resolutions().iter().filter(|target| {
        target.kind() == PackageTargetKind::Executable
            && input.root_packages().contains(target.module().package())
    });
    let declaration = select_candidate(candidates, selector, |target| target.name())
        .map_err(ValueAccessEntryError::Selection)?;
    let module = declaration.module();
    let headers = accepted.headers();
    let invalid_main = |rule| ValueAccessEntryError::InvalidMain {
        executable: declaration.name().into(),
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
        target: input.target(),
        accepted,
        entry,
        result,
    })
}
