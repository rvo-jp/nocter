use std::collections::BTreeMap;
use std::fmt;

use nocter_mir::{MirBody, MirProgram, MirRoot};
use nocter_model::{ExecutableItemId, MirOperationId, MirPlaceId, TestId, TypeId};

use crate::identity::{MachineId, MachineTable};
use crate::{
    MachineAbiError, MachineAbiPlan, MachineContextError, MachineContextPlans, MachineDataTable,
    MachineDestructionId, MachineDestructionTable, MachineFunction, MachineFunctionId,
    MachineFunctionKind, MachineLayoutError, MachineLayoutStore, MachineLinkageError,
    MachineLinkageId, MachineLinkageKey, MachineLinkageTable, MachineProgram, MachineProgramRoot,
    MachineTestProgram,
};

mod address;
mod aggregate;
mod body;
mod call;
mod context;
mod control;
pub(crate) mod destruction;
mod operation;
mod pack;
mod structural;

use body::lower_body;
use context::ProgramLoweringContext;

impl MachineProgram {
    /// Lowers one validated MIR program into an independent target-machine program.
    ///
    /// # Errors
    ///
    /// Returns a compiler-integrity error when layout, ABI, linkage, or a machine operation cannot
    /// be materialized. MIR is never retained as an escape hatch in the resulting program.
    pub fn lower(program: &MirProgram) -> Result<Self, MachineProgramError> {
        let layouts = MachineLayoutStore::build(program)?;
        let abi = MachineAbiPlan::build(program, &layouts)?;
        let linkage = MachineLinkageTable::build(program)?;
        let data = MachineDataTable::build(program);
        let source_domains = assign_function_domains(&linkage)?;
        let destructions =
            MachineDestructionTable::build(program, &layouts, &linkage, &source_domains.items)?;
        let linkage = linkage.with_destructions(&destructions)?;
        let domains = assign_function_domains(&linkage)?;
        let linkage_entries = linkage.iter().collect::<Vec<_>>();

        let functions = linkage_entries
            .into_iter()
            .map(|(linkage_id, entry)| match entry.key() {
                MachineLinkageKey::Destruction(destruction) => {
                    let destruction = destructions
                        .get(destruction)
                        .ok_or(MachineProgramError::MissingDestruction(destruction))?;
                    crate::generated_destruction::generate_destruction_function(
                        linkage_id,
                        destruction.plan(),
                        destruction.abi(),
                        program.executable().types(),
                        &layouts,
                    )
                }
                key => {
                    let (kind, body) = function_source(program, &abi, key)?;
                    let body = lower_body(
                        linkage_id,
                        body,
                        ProgramLoweringContext {
                            types: program.executable().types(),
                            layouts: &layouts,
                            abi: &abi,
                            data: &data,
                            functions: &domains.items,
                            destructions: &destructions,
                            destruction_functions: &domains.destructions,
                        },
                    )?;
                    MachineFunction::new(linkage_id, kind, body).map_err(|error| {
                        MachineProgramError::Dataflow {
                            owner: linkage_id,
                            error,
                        }
                    })
                }
            })
            .collect::<Result<Vec<_>, MachineProgramError>>()?;
        let root = lower_root(program, linkage.root(), &domains.linkages)?;
        let functions = MachineTable::from_values(functions);
        let contexts = MachineContextPlans::build(&functions)?;

        Ok(Self::new(crate::program::MachineProgramParts {
            layouts,
            abi,
            contexts,
            destructions,
            linkage,
            data,
            functions,
            functions_by_linkage: domains.linkages,
            root,
        }))
    }
}

fn function_source<'program>(
    program: &'program MirProgram,
    abi: &MachineAbiPlan,
    key: MachineLinkageKey,
) -> Result<(MachineFunctionKind, &'program MirBody), MachineProgramError> {
    match key {
        MachineLinkageKey::Item(item) => {
            let function = program
                .functions()
                .get(item)
                .ok_or(MachineProgramError::MissingItem(item))?;
            let callable = abi
                .get(item)
                .cloned()
                .ok_or(MachineProgramError::MissingCallableAbi(item))?;
            Ok((MachineFunctionKind::Callable(callable), function.body()))
        }
        MachineLinkageKey::ProcessRoot(target) => {
            let MirRoot::Process(root) = program.root() else {
                return Err(MachineProgramError::MissingProcessRoot(target));
            };
            if root.target() != target {
                return Err(MachineProgramError::MissingProcessRoot(target));
            }
            Ok((MachineFunctionKind::ProcessRoot, root.body()))
        }
        MachineLinkageKey::TestRoot(declaration) => {
            let MirRoot::Tests { cases, .. } = program.root() else {
                return Err(MachineProgramError::MissingTestRoot(declaration));
            };
            let root = cases
                .iter()
                .find(|root| root.declaration() == declaration)
                .ok_or(MachineProgramError::MissingTestRoot(declaration))?;
            Ok((
                MachineFunctionKind::TestRoot {
                    declaration,
                    name: root.name(),
                },
                root.body(),
            ))
        }
        MachineLinkageKey::Destruction(destruction) => {
            Err(MachineProgramError::MissingDestruction(destruction))
        }
    }
}

struct FunctionDomains {
    linkages: BTreeMap<MachineLinkageId, MachineFunctionId>,
    items: BTreeMap<ExecutableItemId, MachineFunctionId>,
    destructions: BTreeMap<MachineDestructionId, MachineFunctionId>,
}

fn assign_function_domains(
    linkage: &MachineLinkageTable,
) -> Result<FunctionDomains, MachineProgramError> {
    let mut by_linkage = BTreeMap::new();
    let mut by_item = BTreeMap::new();
    let mut by_destruction = BTreeMap::new();
    for (index, (linkage_id, entry)) in linkage.iter().enumerate() {
        let function = MachineFunctionId::new(index);
        if by_linkage.insert(linkage_id, function).is_some() {
            return Err(MachineProgramError::DuplicateFunctionLinkage(linkage_id));
        }
        match entry.key() {
            MachineLinkageKey::Item(item) => {
                if by_item.insert(item, function).is_some() {
                    return Err(MachineProgramError::DuplicateItemFunction(item));
                }
            }
            MachineLinkageKey::Destruction(destruction) => {
                if by_destruction.insert(destruction, function).is_some() {
                    return Err(MachineProgramError::DuplicateDestructionFunction(
                        destruction,
                    ));
                }
            }
            MachineLinkageKey::ProcessRoot(_) | MachineLinkageKey::TestRoot(_) => {}
        }
    }
    Ok(FunctionDomains {
        linkages: by_linkage,
        items: by_item,
        destructions: by_destruction,
    })
}

fn lower_root(
    program: &MirProgram,
    root: &crate::MachineRootLinkage,
    functions: &BTreeMap<MachineLinkageId, MachineFunctionId>,
) -> Result<MachineProgramRoot, MachineProgramError> {
    match root {
        crate::MachineRootLinkage::Process { process, entry, .. } => {
            Ok(MachineProgramRoot::Process {
                root: require_function(functions, *process)?,
                entry: require_function(functions, *entry)?,
            })
        }
        crate::MachineRootLinkage::Tests { cases, .. } => cases
            .iter()
            .enumerate()
            .map(|(index, case)| {
                let name = program
                    .executable()
                    .target()
                    .checked()
                    .graph()
                    .symbols()
                    .spelling(case.name())
                    .ok_or(MachineProgramError::MissingTestName(case.declaration()))?;
                Ok(MachineTestProgram::new(
                    crate::MachineTestId::new(index),
                    name,
                    require_function(functions, case.test())?,
                    require_function(functions, case.body())?,
                ))
            })
            .collect::<Result<Vec<_>, MachineProgramError>>()
            .map(|cases| MachineProgramRoot::Tests(cases.into_boxed_slice())),
    }
}

fn require_function(
    functions: &BTreeMap<MachineLinkageId, MachineFunctionId>,
    linkage: MachineLinkageId,
) -> Result<MachineFunctionId, MachineProgramError> {
    functions
        .get(&linkage)
        .copied()
        .ok_or(MachineProgramError::MissingFunctionLinkage(linkage))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineAddressError {
    InvalidRoot,
    InvalidProjection,
    OffsetOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineAggregateError {
    InvalidLayout,
    MemberMismatch,
    OffsetOverflow,
}

#[derive(Debug)]
pub enum MachineProgramError {
    Layout(MachineLayoutError),
    Abi(MachineAbiError),
    Linkage(MachineLinkageError),
    Context(MachineContextError),
    Dataflow {
        owner: MachineLinkageId,
        error: crate::MachineDataflowError,
    },
    DuplicateFunctionLinkage(MachineLinkageId),
    DuplicateItemFunction(ExecutableItemId),
    DuplicateDestructionFunction(MachineDestructionId),
    DuplicateDestructionCall(MachineLinkageId, MirOperationId),
    DuplicatePackDestruction {
        owner: MachineLinkageId,
        operation: MirOperationId,
        segment: usize,
    },
    MissingFunctionLinkage(MachineLinkageId),
    MissingItemFunction(ExecutableItemId),
    MissingItem(ExecutableItemId),
    MissingCallableAbi(ExecutableItemId),
    MissingProcessRoot(nocter_model::PackageTargetId),
    MissingTestRoot(TestId),
    MissingTestName(TestId),
    MissingLinkageKey(MachineLinkageKey),
    MissingDestruction(crate::MachineDestructionId),
    MissingBytePointerType,
    MissingPackDestruction {
        owner: MachineLinkageId,
        operation: MirOperationId,
        segment: usize,
    },
    InvalidDestructionAbi(MachineLinkageId),
    InvalidGeneratedDestruction(MachineLinkageId, crate::MachineBlockId),
    MissingGeneratedDestruction(MachineLinkageId, MirOperationId),
    MissingStoredLayout(TypeId),
    MissingStaticText(Box<str>),
    MissingBodyIdentity {
        owner: MachineLinkageId,
        source: Box<str>,
    },
    Address {
        owner: MachineLinkageId,
        place: MirPlaceId,
        error: MachineAddressError,
    },
    Aggregate {
        owner: MachineLinkageId,
        operation: MirOperationId,
        error: MachineAggregateError,
    },
    Destruction {
        owner: MachineLinkageId,
        operation: MirOperationId,
        error: crate::MachineDestructionError,
    },
    Structural {
        owner: MachineLinkageId,
        operation: MirOperationId,
        error: crate::MachineStructuralError,
    },
    InvalidPackTarget {
        owner: MachineLinkageId,
        operation: MirOperationId,
    },
    InvalidPackReceiver {
        owner: MachineLinkageId,
        operation: MirOperationId,
    },
    MissingOperationResult {
        owner: MachineLinkageId,
        operation: MirOperationId,
    },
    UnsupportedPlaceSwitch(MachineLinkageId),
    InvalidValueSwitch(MachineLinkageId),
    InvalidTagSwitch(MachineLinkageId),
}

impl fmt::Display for MachineProgramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "machine program construction failed: {self:?}")
    }
}

impl std::error::Error for MachineProgramError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error),
            Self::Abi(error) => Some(error),
            Self::Linkage(error) => Some(error),
            Self::Context(error) => Some(error),
            Self::Dataflow { error, .. } => Some(error),
            Self::DuplicateFunctionLinkage(_)
            | Self::DuplicateItemFunction(_)
            | Self::DuplicateDestructionFunction(_)
            | Self::DuplicateDestructionCall(_, _)
            | Self::DuplicatePackDestruction { .. }
            | Self::MissingFunctionLinkage(_)
            | Self::MissingItemFunction(_)
            | Self::MissingItem(_)
            | Self::MissingCallableAbi(_)
            | Self::MissingProcessRoot(_)
            | Self::MissingTestRoot(_)
            | Self::MissingTestName(_)
            | Self::MissingLinkageKey(_)
            | Self::MissingDestruction(_)
            | Self::MissingBytePointerType
            | Self::MissingPackDestruction { .. }
            | Self::InvalidDestructionAbi(_)
            | Self::InvalidGeneratedDestruction(_, _)
            | Self::MissingGeneratedDestruction(_, _)
            | Self::MissingStoredLayout(_)
            | Self::MissingStaticText(_)
            | Self::MissingBodyIdentity { .. }
            | Self::Address { .. }
            | Self::Aggregate { .. }
            | Self::Destruction { .. }
            | Self::Structural { .. }
            | Self::InvalidPackTarget { .. }
            | Self::InvalidPackReceiver { .. }
            | Self::MissingOperationResult { .. }
            | Self::UnsupportedPlaceSwitch(_)
            | Self::InvalidValueSwitch(_)
            | Self::InvalidTagSwitch(_) => None,
        }
    }
}

impl From<MachineLayoutError> for MachineProgramError {
    fn from(error: MachineLayoutError) -> Self {
        Self::Layout(error)
    }
}

impl From<MachineAbiError> for MachineProgramError {
    fn from(error: MachineAbiError) -> Self {
        Self::Abi(error)
    }
}

impl From<MachineLinkageError> for MachineProgramError {
    fn from(error: MachineLinkageError) -> Self {
        Self::Linkage(error)
    }
}

impl From<MachineContextError> for MachineProgramError {
    fn from(error: MachineContextError) -> Self {
        Self::Context(error)
    }
}
