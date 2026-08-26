use std::fmt;

use nocter_mir::{MirBody, MirProgram, MirRoot};
use nocter_model::{ExecutableItemId, MirOperationId, MirPlaceId, TestId, TypeId};

use crate::destruction_table::MachineDestructionPlanTable;
use crate::identity::{MachineId, MachineTable};
use crate::linkage::{MachineLinkagePlan, MachineRootLinkage};
use crate::{
    MachineAbiError, MachineAbiPlan, MachineContextError, MachineContextPlans, MachineFunction,
    MachineFunctionId, MachineFunctionKind, MachineLayoutError, MachineLayoutPlan,
    MachineLinkageError, MachineLinkageId, MachineLinkageKey, MachineProgram, MachineProgramRoot,
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
        let layouts = MachineLayoutPlan::build(program)?;
        let abi = MachineAbiPlan::build(program, &layouts)?;
        let linkage = MachineLinkagePlan::build(program)?;
        let data = crate::data::MachineDataPlan::build(program);
        let source_functions = crate::function_domain::MachineFunctionDomain::new(&linkage);
        let destructions =
            MachineDestructionPlanTable::build(program, &layouts, &linkage, source_functions)?;
        let linkage = linkage.with_destructions(&destructions)?;
        let function_domain = crate::function_domain::MachineFunctionDomain::new(&linkage);
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
                        program.types(),
                        &layouts,
                    )
                }
                key => {
                    let (kind, body) = function_source(program, &abi, key)?;
                    let body = lower_body(
                        linkage_id,
                        body,
                        ProgramLoweringContext {
                            types: program.types(),
                            layouts: &layouts,
                            abi: &abi,
                            data: &data,
                            functions: function_domain,
                            destructions: &destructions,
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
        let root = lower_root(linkage.root(), function_domain)?;
        let function_table = MachineTable::from_values(functions);
        let contexts = MachineContextPlans::build(&function_table)?;
        let primitive_abis = abi.finish();
        let data = data.finish();
        let layouts = layouts.finish();

        Ok(Self::new(crate::program::MachineProgramParts {
            layouts,
            primitive_abis,
            contexts,
            data,
            functions: function_table,
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
            Ok((MachineFunctionKind::TestRoot, root.body()))
        }
        MachineLinkageKey::Destruction(destruction) => {
            Err(MachineProgramError::MissingDestruction(destruction))
        }
    }
}

fn lower_root(
    root: &MachineRootLinkage,
    functions: crate::function_domain::MachineFunctionDomain<'_>,
) -> Result<MachineProgramRoot, MachineProgramError> {
    match root {
        MachineRootLinkage::Process { process, entry, .. } => Ok(MachineProgramRoot::Process {
            root: require_function(functions, *process)?,
            entry: require_function(functions, *entry)?,
        }),
        MachineRootLinkage::Tests { cases, .. } => cases
            .iter()
            .enumerate()
            .map(|(index, case)| {
                Ok(MachineTestProgram::new(
                    crate::MachineTestId::new(index),
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
    functions: crate::function_domain::MachineFunctionDomain<'_>,
    linkage: MachineLinkageId,
) -> Result<MachineFunctionId, MachineProgramError> {
    functions
        .for_linkage(linkage)
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
    MissingPrimitiveAbi(MirOperationId),
    MissingProcessRoot(nocter_model::PackageTargetId),
    MissingTestRoot(TestId),
    MissingLinkageKey(MachineLinkageKey),
    MissingDestruction(crate::MachineDestructionId),
    MissingBytePointerType,
    MissingRuntimePrimitive(nocter_runtime_contract::RuntimePrimitive),
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
            Self::DuplicateDestructionCall(_, _)
            | Self::DuplicatePackDestruction { .. }
            | Self::MissingFunctionLinkage(_)
            | Self::MissingItemFunction(_)
            | Self::MissingItem(_)
            | Self::MissingCallableAbi(_)
            | Self::MissingPrimitiveAbi(_)
            | Self::MissingProcessRoot(_)
            | Self::MissingTestRoot(_)
            | Self::MissingLinkageKey(_)
            | Self::MissingDestruction(_)
            | Self::MissingBytePointerType
            | Self::MissingRuntimePrimitive(_)
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
