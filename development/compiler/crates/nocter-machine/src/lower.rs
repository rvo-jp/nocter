use std::collections::BTreeMap;
use std::fmt;

use nocter_mir::{MirBody, MirProgram, MirRoot};
use nocter_model::{ExecutableItemId, MirOperationId, MirPlaceId, TestId, TypeId};

use crate::identity::{MachineId, MachineTable};
use crate::{
    MachineAbiError, MachineAbiPlan, MachineAllocationError, MachineAllocationPlan,
    MachineDataTable, MachineFunction, MachineFunctionId, MachineFunctionKind, MachineLayoutError,
    MachineLayoutStore, MachineLinkageError, MachineLinkageId, MachineLinkageKey,
    MachineLinkageTable, MachineProgram, MachineProgramRoot, MachineTestProgram,
};

mod address;
mod aggregate;
mod body;
mod call;
mod control;
mod destruction;
mod operation;
mod pack;
mod structural;

use body::lower_body;

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

        let mut functions_by_linkage = BTreeMap::new();
        let mut functions_by_item = BTreeMap::new();
        let linkage_entries = linkage.iter().collect::<Vec<_>>();
        for (index, (linkage_id, entry)) in linkage_entries.iter().copied().enumerate() {
            let function = MachineFunctionId::new(index);
            if functions_by_linkage.insert(linkage_id, function).is_some() {
                return Err(MachineProgramError::DuplicateFunctionLinkage(linkage_id));
            }
            if let MachineLinkageKey::Item(item) = entry.key()
                && functions_by_item.insert(item, function).is_some()
            {
                return Err(MachineProgramError::DuplicateItemFunction(item));
            }
        }

        let functions = linkage_entries
            .into_iter()
            .map(|(linkage_id, entry)| {
                let (kind, body) = function_source(program, &abi, entry.key())?;
                let body = lower_body(
                    linkage_id,
                    body,
                    program.executable().types(),
                    &layouts,
                    &data,
                    &functions_by_item,
                )?;
                Ok(MachineFunction::new(linkage_id, kind, body))
            })
            .collect::<Result<Vec<_>, MachineProgramError>>()?;
        let root = lower_root(linkage.root(), &functions_by_linkage)?;
        let functions = MachineTable::from_values(functions);
        let allocation = MachineAllocationPlan::build(&functions)?;

        Ok(Self::new(crate::program::MachineProgramParts {
            layouts,
            abi,
            allocation,
            linkage,
            data,
            functions,
            functions_by_linkage,
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
    }
}

fn lower_root(
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
            .map(|case| {
                Ok(MachineTestProgram::new(
                    case.declaration(),
                    case.name(),
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
    Allocation(MachineAllocationError),
    DuplicateFunctionLinkage(MachineLinkageId),
    DuplicateItemFunction(ExecutableItemId),
    MissingFunctionLinkage(MachineLinkageId),
    MissingItemFunction(ExecutableItemId),
    MissingItem(ExecutableItemId),
    MissingCallableAbi(ExecutableItemId),
    MissingProcessRoot(nocter_model::PackageTargetId),
    MissingTestRoot(TestId),
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
            Self::Allocation(error) => Some(error),
            Self::DuplicateFunctionLinkage(_)
            | Self::DuplicateItemFunction(_)
            | Self::MissingFunctionLinkage(_)
            | Self::MissingItemFunction(_)
            | Self::MissingItem(_)
            | Self::MissingCallableAbi(_)
            | Self::MissingProcessRoot(_)
            | Self::MissingTestRoot(_)
            | Self::MissingStoredLayout(_)
            | Self::MissingStaticText(_)
            | Self::MissingBodyIdentity { .. }
            | Self::Address { .. }
            | Self::Aggregate { .. }
            | Self::Destruction { .. }
            | Self::Structural { .. }
            | Self::InvalidPackTarget { .. }
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

impl From<MachineAllocationError> for MachineProgramError {
    fn from(error: MachineAllocationError) -> Self {
        Self::Allocation(error)
    }
}
