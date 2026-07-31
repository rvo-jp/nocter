use super::*;

pub(super) fn unresolved_call_target_diagnostic(function: &FunctionSymbol) -> Diagnostic {
    Diagnostic::error(
        "E9002",
        format!(
            "codegen could not resolve function `{}`",
            function.description()
        ),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum FunctionSymbol {
    SameFile(String),
    Imported {
        source: crate::source::SourceId,
        name: String,
    },
}

impl FunctionSymbol {
    pub(super) fn same_file(name: impl Into<String>) -> Self {
        Self::SameFile(name.into())
    }

    pub(super) fn from_function(function: &Function) -> Self {
        Self::from_call_target(&function.target)
    }

    pub(super) fn from_call_target(target: &CallTarget) -> Self {
        match target {
            CallTarget::SameFile(name) => Self::same_file(name),
            CallTarget::Imported { source, name } => Self::Imported {
                source: *source,
                name: name.clone(),
            },
        }
    }

    pub(super) fn description(&self) -> String {
        match self {
            Self::SameFile(name) => name.clone(),
            Self::Imported { source, name } => {
                format!("{} from source {}", name, source.raw())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DataAddressPatch {
    pub(super) instruction_offset: usize,
    pub(super) register: XReg,
    pub(super) data_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FunctionCallPatch {
    pub(super) instruction_offset: usize,
    pub(super) function: FunctionSymbol,
}
