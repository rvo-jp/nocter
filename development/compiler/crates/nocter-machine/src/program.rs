use std::collections::BTreeMap;

use nocter_model::{Symbol, TestId};

use crate::identity::MachineTable;
use crate::{
    MachineAbiPlan, MachineAddress, MachineAddressId, MachineBlock, MachineBlockId,
    MachineDataTable, MachineDropFlag, MachineDropFlagId, MachineFunctionId, MachineLayoutStore,
    MachineLinkageId, MachineLinkageTable, MachineOperation, MachineOperationId, MachinePack,
    MachinePackId, MachineStackId, MachineStackObject, MachineValue, MachineValueId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineFunctionKind {
    Callable(crate::MachineCallableAbi),
    ProcessRoot,
    TestRoot { declaration: TestId, name: Symbol },
}

/// One target-independent function body with body-local dense identity domains.
#[derive(Debug)]
pub struct MachineBody {
    parameters: Box<[MachineStackId]>,
    stack: MachineTable<MachineStackId, MachineStackObject>,
    drop_flags: MachineTable<MachineDropFlagId, MachineDropFlag>,
    addresses: MachineTable<MachineAddressId, MachineAddress>,
    values: MachineTable<MachineValueId, MachineValue>,
    operations: MachineTable<MachineOperationId, MachineOperation>,
    packs: MachineTable<MachinePackId, MachinePack>,
    blocks: MachineTable<MachineBlockId, MachineBlock>,
    entry: MachineBlockId,
}

pub(crate) struct MachineBodyDomains {
    pub(crate) stack: MachineTable<MachineStackId, MachineStackObject>,
    pub(crate) drop_flags: MachineTable<MachineDropFlagId, MachineDropFlag>,
    pub(crate) addresses: MachineTable<MachineAddressId, MachineAddress>,
    pub(crate) values: MachineTable<MachineValueId, MachineValue>,
    pub(crate) operations: MachineTable<MachineOperationId, MachineOperation>,
    pub(crate) packs: MachineTable<MachinePackId, MachinePack>,
    pub(crate) blocks: MachineTable<MachineBlockId, MachineBlock>,
}

impl MachineBody {
    pub(crate) fn new(
        parameters: impl Into<Box<[MachineStackId]>>,
        domains: MachineBodyDomains,
        entry: MachineBlockId,
    ) -> Self {
        Self {
            parameters: parameters.into(),
            stack: domains.stack,
            drop_flags: domains.drop_flags,
            addresses: domains.addresses,
            values: domains.values,
            operations: domains.operations,
            packs: domains.packs,
            blocks: domains.blocks,
            entry,
        }
    }

    #[must_use]
    pub const fn parameters(&self) -> &[MachineStackId] {
        &self.parameters
    }

    #[must_use]
    pub fn stack(&self, id: MachineStackId) -> Option<MachineStackObject> {
        self.stack.get(id).copied()
    }

    #[must_use]
    pub fn stack_objects(
        &self,
    ) -> impl ExactSizeIterator<Item = (MachineStackId, MachineStackObject)> + '_ {
        self.stack.iter().map(|(id, object)| (id, *object))
    }

    #[must_use]
    pub fn drop_flag(&self, id: MachineDropFlagId) -> Option<MachineDropFlag> {
        self.drop_flags.get(id).copied()
    }

    #[must_use]
    pub fn drop_flags(
        &self,
    ) -> impl ExactSizeIterator<Item = (MachineDropFlagId, MachineDropFlag)> + '_ {
        self.drop_flags.iter().map(|(id, flag)| (id, *flag))
    }

    #[must_use]
    pub fn address(&self, id: MachineAddressId) -> Option<&MachineAddress> {
        self.addresses.get(id)
    }

    #[must_use]
    pub fn addresses(&self) -> impl ExactSizeIterator<Item = (MachineAddressId, &MachineAddress)> {
        self.addresses.iter()
    }

    #[must_use]
    pub fn value(&self, id: MachineValueId) -> Option<MachineValue> {
        self.values.get(id).copied()
    }

    #[must_use]
    pub fn values(&self) -> impl ExactSizeIterator<Item = (MachineValueId, MachineValue)> + '_ {
        self.values.iter().map(|(id, value)| (id, *value))
    }

    #[must_use]
    pub fn operation(&self, id: MachineOperationId) -> Option<&MachineOperation> {
        self.operations.get(id)
    }

    #[must_use]
    pub fn operations(
        &self,
    ) -> impl ExactSizeIterator<Item = (MachineOperationId, &MachineOperation)> {
        self.operations.iter()
    }

    #[must_use]
    pub fn pack(&self, id: MachinePackId) -> Option<&MachinePack> {
        self.packs.get(id)
    }

    #[must_use]
    pub fn packs(&self) -> impl ExactSizeIterator<Item = (MachinePackId, &MachinePack)> {
        self.packs.iter()
    }

    #[must_use]
    pub fn block(&self, id: MachineBlockId) -> Option<&MachineBlock> {
        self.blocks.get(id)
    }

    #[must_use]
    pub fn blocks(&self) -> impl ExactSizeIterator<Item = (MachineBlockId, &MachineBlock)> {
        self.blocks.iter()
    }

    #[must_use]
    pub const fn entry(&self) -> MachineBlockId {
        self.entry
    }
}

#[derive(Debug)]
pub struct MachineFunction {
    linkage: MachineLinkageId,
    kind: MachineFunctionKind,
    body: MachineBody,
    dataflow: crate::MachineFunctionDataflow,
}

impl MachineFunction {
    pub(crate) fn new(
        linkage: MachineLinkageId,
        kind: MachineFunctionKind,
        body: MachineBody,
    ) -> Result<Self, crate::MachineDataflowError> {
        let dataflow = crate::MachineFunctionDataflow::build(&body)?;
        Ok(Self {
            linkage,
            kind,
            body,
            dataflow,
        })
    }

    #[must_use]
    pub const fn linkage(&self) -> MachineLinkageId {
        self.linkage
    }

    #[must_use]
    pub const fn kind(&self) -> &MachineFunctionKind {
        &self.kind
    }

    #[must_use]
    pub const fn body(&self) -> &MachineBody {
        &self.body
    }

    #[must_use]
    pub const fn dataflow(&self) -> &crate::MachineFunctionDataflow {
        &self.dataflow
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineTestProgram {
    declaration: TestId,
    name: Symbol,
    root: MachineFunctionId,
    body: MachineFunctionId,
}

impl MachineTestProgram {
    pub(crate) const fn new(
        declaration: TestId,
        name: Symbol,
        root: MachineFunctionId,
        body: MachineFunctionId,
    ) -> Self {
        Self {
            declaration,
            name,
            root,
            body,
        }
    }

    #[must_use]
    pub const fn declaration(self) -> TestId {
        self.declaration
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn root(self) -> MachineFunctionId {
        self.root
    }

    #[must_use]
    pub const fn body(self) -> MachineFunctionId {
        self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineProgramRoot {
    Process {
        root: MachineFunctionId,
        entry: MachineFunctionId,
    },
    Tests(Box<[MachineTestProgram]>),
}

/// A complete target-independent machine program. No source or MIR identity is needed to walk it.
#[derive(Debug)]
pub struct MachineProgram {
    layouts: MachineLayoutStore,
    abi: MachineAbiPlan,
    allocation: crate::MachineAllocationPlan,
    linkage: MachineLinkageTable,
    data: MachineDataTable,
    functions: MachineTable<MachineFunctionId, MachineFunction>,
    functions_by_linkage: BTreeMap<MachineLinkageId, MachineFunctionId>,
    root: MachineProgramRoot,
}

pub(crate) struct MachineProgramParts {
    pub(crate) layouts: MachineLayoutStore,
    pub(crate) abi: MachineAbiPlan,
    pub(crate) allocation: crate::MachineAllocationPlan,
    pub(crate) linkage: MachineLinkageTable,
    pub(crate) data: MachineDataTable,
    pub(crate) functions: MachineTable<MachineFunctionId, MachineFunction>,
    pub(crate) functions_by_linkage: BTreeMap<MachineLinkageId, MachineFunctionId>,
    pub(crate) root: MachineProgramRoot,
}

impl MachineProgram {
    pub(crate) fn new(parts: MachineProgramParts) -> Self {
        Self {
            layouts: parts.layouts,
            abi: parts.abi,
            allocation: parts.allocation,
            linkage: parts.linkage,
            data: parts.data,
            functions: parts.functions,
            functions_by_linkage: parts.functions_by_linkage,
            root: parts.root,
        }
    }

    #[must_use]
    pub const fn layouts(&self) -> &MachineLayoutStore {
        &self.layouts
    }

    #[must_use]
    pub const fn abi(&self) -> &MachineAbiPlan {
        &self.abi
    }

    #[must_use]
    pub const fn allocation(&self) -> &crate::MachineAllocationPlan {
        &self.allocation
    }

    #[must_use]
    pub const fn linkage(&self) -> &MachineLinkageTable {
        &self.linkage
    }

    #[must_use]
    pub const fn data(&self) -> &MachineDataTable {
        &self.data
    }

    #[must_use]
    pub fn function(&self, id: MachineFunctionId) -> Option<&MachineFunction> {
        self.functions.get(id)
    }

    #[must_use]
    pub fn function_for_linkage(&self, linkage: MachineLinkageId) -> Option<MachineFunctionId> {
        self.functions_by_linkage.get(&linkage).copied()
    }

    #[must_use]
    pub fn functions(
        &self,
    ) -> impl ExactSizeIterator<Item = (MachineFunctionId, &MachineFunction)> {
        self.functions.iter()
    }

    #[must_use]
    pub const fn root(&self) -> &MachineProgramRoot {
        &self.root
    }
}
