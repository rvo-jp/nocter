use std::fmt;

use nocter_declarations::{CallableKind, CallableOwner, ExportedEntity, PackageTargetKind};
use nocter_model::{
    BodyId, BuiltinType, CallableId, ModuleId, PackageId, PackageTargetId, TypeId, TypeKind,
};

use crate::TargetProgram;

/// The successful payload accepted by the generated process-entry wrapper.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProcessSuccessType {
    Void,
    I32,
    Usize,
}

/// The exact source-level result contract accepted for an executable entry.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProcessResultContract {
    success: ProcessSuccessType,
    fallible: bool,
}

impl ProcessResultContract {
    #[must_use]
    pub const fn success(self) -> ProcessSuccessType {
        self.success
    }

    #[must_use]
    pub const fn is_fallible(self) -> bool {
        self.fallible
    }
}

/// One exact selected executable root after module-local entry validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableEntry {
    target: PackageTargetId,
    package: PackageId,
    module: ModuleId,
    callable: CallableId,
    body: BodyId,
    result_type: TypeId,
    process_result: ProcessResultContract,
}

impl ExecutableEntry {
    #[must_use]
    pub const fn target(self) -> PackageTargetId {
        self.target
    }

    #[must_use]
    pub const fn package(self) -> PackageId {
        self.package
    }

    #[must_use]
    pub const fn module(self) -> ModuleId {
        self.module
    }

    #[must_use]
    pub const fn callable(self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn body(self) -> BodyId {
        self.body
    }

    #[must_use]
    pub const fn result_type(self) -> TypeId {
        self.result_type
    }

    #[must_use]
    pub const fn process_result(self) -> ProcessResultContract {
        self.process_result
    }
}

/// The part of the selected top-level `main` contract that is invalid.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EntryContractRule {
    TopLevelFunction,
    GenericParameters,
    ValueParameters,
    MissingBody,
    ResultType,
}

/// Failure to derive one executable root from a validated target program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntrySelectionError {
    UnknownTarget(PackageTargetId),
    NotExecutable(PackageTargetId),
    MissingMain {
        target: PackageTargetId,
        module: ModuleId,
    },
    InvalidMainEntity {
        target: PackageTargetId,
        entity: ExportedEntity,
    },
    InvalidMainContract {
        target: PackageTargetId,
        callable: CallableId,
        rule: EntryContractRule,
    },
}

impl fmt::Display for EntrySelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTarget(_) => {
                formatter.write_str("executable selection names an unknown package target")
            }
            Self::NotExecutable(_) => {
                formatter.write_str("selected package target is not executable")
            }
            Self::MissingMain { .. } => {
                formatter.write_str("selected executable module has no top-level main function")
            }
            Self::InvalidMainEntity { .. } => formatter
                .write_str("selected executable module's main name is not a top-level function"),
            Self::InvalidMainContract { rule, .. } => {
                write!(formatter, "selected main violates its {rule:?} contract")
            }
        }
    }
}

impl std::error::Error for EntrySelectionError {}

/// Selects the exact top-level `main` owned by one executable package target.
///
/// Lookup uses only the selected module's authored namespace. Imported modules, prelude fallback,
/// and re-exported callables cannot become the entry by sharing the spelling.
///
/// # Errors
///
/// Returns the first closed entry-selection or entry-contract failure.
pub fn select_executable_entry(
    program: &TargetProgram,
    selected: PackageTargetId,
) -> Result<ExecutableEntry, EntrySelectionError> {
    let checked = program.checked();
    let graph = checked.graph();
    let target = graph
        .package_targets()
        .get(selected)
        .ok_or(EntrySelectionError::UnknownTarget(selected))?;
    if target.kind() != PackageTargetKind::Executable {
        return Err(EntrySelectionError::NotExecutable(selected));
    }
    let main = graph.symbols().get("main").and_then(|name| {
        graph
            .module_namespaces()
            .get(target.module())?
            .lookup_authored(name)
    });
    let Some(entry) = main else {
        return Err(EntrySelectionError::MissingMain {
            target: selected,
            module: target.module(),
        });
    };
    let ExportedEntity::Callable(callable) = entry.target() else {
        return Err(EntrySelectionError::InvalidMainEntity {
            target: selected,
            entity: entry.target(),
        });
    };
    let declaration = graph.declarations().callables().get(callable).ok_or(
        EntrySelectionError::InvalidMainContract {
            target: selected,
            callable,
            rule: EntryContractRule::TopLevelFunction,
        },
    )?;
    if declaration.kind() != CallableKind::Function
        || declaration.owner() != CallableOwner::Module(target.module())
        || declaration.receiver().is_some()
    {
        return Err(invalid_contract(
            selected,
            callable,
            EntryContractRule::TopLevelFunction,
        ));
    }
    if !declaration.generic_parameters().is_empty() {
        return Err(invalid_contract(
            selected,
            callable,
            EntryContractRule::GenericParameters,
        ));
    }
    if !declaration.parameters().is_empty() {
        return Err(invalid_contract(
            selected,
            callable,
            EntryContractRule::ValueParameters,
        ));
    }
    let body = declaration
        .body()
        .ok_or_else(|| invalid_contract(selected, callable, EntryContractRule::MissingBody))?;
    let process_result = process_result(checked.types(), declaration.result())
        .ok_or_else(|| invalid_contract(selected, callable, EntryContractRule::ResultType))?;
    Ok(ExecutableEntry {
        target: selected,
        package: target.package(),
        module: target.module(),
        callable,
        body,
        result_type: declaration.result(),
        process_result,
    })
}

const fn invalid_contract(
    target: PackageTargetId,
    callable: CallableId,
    rule: EntryContractRule,
) -> EntrySelectionError {
    EntrySelectionError::InvalidMainContract {
        target,
        callable,
        rule,
    }
}

fn process_result(
    types: &nocter_model::TypeStore,
    result: TypeId,
) -> Option<ProcessResultContract> {
    let (base, fallible) = match types.get(result)? {
        TypeKind::Fallible(payload) => (*payload, true),
        _ => (result, false),
    };
    let success = match types.get(base)? {
        TypeKind::Builtin(BuiltinType::Void) => ProcessSuccessType::Void,
        TypeKind::Builtin(BuiltinType::I32) => ProcessSuccessType::I32,
        TypeKind::Builtin(BuiltinType::Usize) => ProcessSuccessType::Usize,
        _ => return None,
    };
    Some(ProcessResultContract { success, fallible })
}
