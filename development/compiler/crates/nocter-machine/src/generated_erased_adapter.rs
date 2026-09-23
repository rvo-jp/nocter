use nocter_model::{CallableCapability, TypeId};
use nocter_runtime_contract::{RuntimePrimitive, RuntimeType, RuntimeTypeTable};

use crate::identity::MachineId;
use crate::{
    MachineAddress, MachineAddressRoot, MachineBlock, MachineBlockId, MachineCall,
    MachineCallAllocation, MachineCallTarget, MachineCallableAbi, MachineConstant, MachineFunction,
    MachineFunctionKind, MachineLayoutPlan, MachineLinkageId, MachineOperation, MachineOperationId,
    MachineOperationKind, MachineResultAbi, MachineStackId, MachineStackObject,
    MachineStackPurpose, MachineTerminator, MachineValue, MachineValueDefinition, MachineValueId,
    MachineValueRepresentation,
};

/// Builds the one-shot ABI adapter stored by an `any func` value.
///
/// The adapter is the sole owner of conversion from the uniform erased `(environment pointer,
/// arguments...)` ABI to the concrete closure ABI. It preserves the result, completes any
/// destruction not owned by the concrete body, and releases the environment mapping before
/// returning.
pub(crate) fn generate_erased_adapter(
    linkage: MachineLinkageId,
    adapter: &crate::erased_adapter::MachineErasedAdapter,
    types: &RuntimeTypeTable,
    layouts: &MachineLayoutPlan,
    functions: crate::function_domain::MachineFunctionDomain<'_>,
    destructions: &crate::destruction_table::MachineDestructionPlanTable,
) -> Result<MachineFunction, crate::MachineProgramError> {
    let target = functions.for_item(adapter.target()).ok_or(
        crate::MachineProgramError::MissingItemFunction(adapter.target()),
    )?;
    let destroy = adapter
        .destruction()
        .map(|destruction| {
            destructions
                .get(destruction)
                .ok_or(crate::MachineProgramError::MissingDestruction(destruction))?;
            functions
                .for_destruction(destruction)
                .ok_or(crate::MachineProgramError::MissingDestruction(destruction))
        })
        .transpose()?;
    let mut builder = AdapterBuilder::new(linkage, adapter.abi(), types, layouts)?;
    let pointer = builder.load_parameter(0)?;
    let environment = builder.environment_argument(
        pointer,
        adapter.environment(),
        adapter.target_environment(),
        adapter.source_capability(),
    )?;
    let mut arguments = vec![environment];
    for position in 1..adapter.abi().arguments().len() {
        arguments.push(builder.load_parameter(position)?);
    }
    let result = builder.call(target, arguments, adapter.abi().result())?;
    if let Some(destroy) = destroy {
        let zero = builder.usize_constant(0)?;
        builder.call_effect(destroy, [pointer, zero]);
    }
    let bytes = builder.usize_constant(adapter.mapped_size())?;
    builder.append_effect(MachineOperationKind::ReleaseMappedStorage { pointer, bytes });
    builder.finish(adapter.abi().clone(), result)
}

struct AdapterBuilder<'a> {
    owner: MachineLinkageId,
    types: &'a RuntimeTypeTable,
    layouts: &'a MachineLayoutPlan,
    parameters: Vec<MachineStackId>,
    stack: Vec<MachineStackObject>,
    addresses: Vec<MachineAddress>,
    values: Vec<MachineValue>,
    operations: Vec<MachineOperation>,
    usize_: TypeId,
}

impl<'a> AdapterBuilder<'a> {
    fn new(
        owner: MachineLinkageId,
        abi: &MachineCallableAbi,
        types: &'a RuntimeTypeTable,
        layouts: &'a MachineLayoutPlan,
    ) -> Result<Self, crate::MachineProgramError> {
        let mut stack = Vec::with_capacity(abi.arguments().len());
        for (position, argument) in abi.arguments().iter().enumerate() {
            let layout = layouts.get(argument.ty()).ok_or(
                crate::MachineProgramError::MissingStoredLayout(argument.ty()),
            )?;
            stack.push(MachineStackObject::new(
                argument.ty(),
                layout.size(),
                layout.alignment(),
                MachineStackPurpose::Parameter { position },
            ));
        }
        Ok(Self {
            owner,
            types,
            layouts,
            parameters: (0..stack.len()).map(MachineStackId::new).collect(),
            stack,
            addresses: Vec::new(),
            values: Vec::new(),
            operations: Vec::new(),
            usize_: types.primitive(RuntimePrimitive::Usize).ok_or(
                crate::MachineProgramError::MissingRuntimePrimitive(RuntimePrimitive::Usize),
            )?,
        })
    }

    fn load_parameter(
        &mut self,
        position: usize,
    ) -> Result<MachineValueId, crate::MachineProgramError> {
        let stack = *self.parameters.get(position).ok_or(
            crate::MachineProgramError::InvalidGeneratedErasedAdapter(self.owner),
        )?;
        let object = self.stack[stack.index()];
        let source = self.add_address(MachineAddress::new(
            object.ty(),
            object.size(),
            object.alignment(),
            MachineAddressRoot::Stack(stack),
            [],
        ));
        self.append_value(object.ty(), MachineOperationKind::Load { source })
    }

    fn environment_argument(
        &mut self,
        pointer: MachineValueId,
        environment: TypeId,
        target_environment: TypeId,
        capability: CallableCapability,
    ) -> Result<MachineValueId, crate::MachineProgramError> {
        let layout = self
            .layouts
            .get(environment)
            .ok_or(crate::MachineProgramError::MissingStoredLayout(environment))?;
        let source = self.add_address(MachineAddress::new(
            environment,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Pointer { value: pointer },
            [],
        ));
        match capability {
            CallableCapability::Owned => {
                if target_environment != environment {
                    return Err(crate::MachineProgramError::InvalidGeneratedErasedAdapter(
                        self.owner,
                    ));
                }
                self.append_value(environment, MachineOperationKind::Load { source })
            }
            CallableCapability::Readonly | CallableCapability::ReadWrite => self.append_value(
                target_environment,
                MachineOperationKind::AddressOf { source },
            ),
        }
    }

    fn call(
        &mut self,
        target: crate::MachineFunctionId,
        arguments: impl Into<Box<[MachineValueId]>>,
        result: MachineResultAbi,
    ) -> Result<Option<MachineValueId>, crate::MachineProgramError> {
        let kind = MachineOperationKind::Call(MachineCall::new(
            MachineCallTarget::Direct(target),
            arguments,
            MachineCallAllocation::Inherit,
            None,
        ));
        match result {
            MachineResultAbi::Completion | MachineResultAbi::Diverging => {
                self.append_effect(kind);
                Ok(None)
            }
            MachineResultAbi::Value(value) => self.append_value(value.ty(), kind).map(Some),
        }
    }

    fn call_effect(
        &mut self,
        target: crate::MachineFunctionId,
        arguments: impl Into<Box<[MachineValueId]>>,
    ) {
        self.append_effect(MachineOperationKind::Call(MachineCall::new(
            MachineCallTarget::Direct(target),
            arguments,
            MachineCallAllocation::Inherit,
            None,
        )));
    }

    fn usize_constant(&mut self, value: u64) -> Result<MachineValueId, crate::MachineProgramError> {
        self.append_value(
            self.usize_,
            MachineOperationKind::Constant(MachineConstant::Integer(i128::from(value))),
        )
    }

    fn add_address(&mut self, address: MachineAddress) -> crate::MachineAddressId {
        let id = crate::MachineAddressId::new(self.addresses.len());
        self.addresses.push(address);
        id
    }

    fn append_effect(&mut self, kind: MachineOperationKind) {
        self.operations.push(MachineOperation::new(kind, None));
    }

    fn append_value(
        &mut self,
        ty: TypeId,
        kind: MachineOperationKind,
    ) -> Result<MachineValueId, crate::MachineProgramError> {
        let operation = MachineOperationId::new(self.operations.len());
        let value = MachineValueId::new(self.values.len());
        self.values.push(MachineValue::new(
            ty,
            self.value_representation(ty)?,
            MachineValueDefinition::Operation(operation),
        ));
        self.operations
            .push(MachineOperation::new(kind, Some(value)));
        Ok(value)
    }

    fn value_representation(
        &self,
        ty: TypeId,
    ) -> Result<MachineValueRepresentation, crate::MachineProgramError> {
        match self.types.get(ty) {
            Some(RuntimeType::Primitive(RuntimePrimitive::Void)) => {
                Ok(MachineValueRepresentation::Completion)
            }
            Some(RuntimeType::Primitive(RuntimePrimitive::Never)) => {
                Ok(MachineValueRepresentation::Diverging)
            }
            Some(_) => self
                .layouts
                .get(ty)
                .and_then(|layout| {
                    self.layouts
                        .class(ty)
                        .map(|class| MachineValueRepresentation::Stored {
                            size: layout.size(),
                            alignment: layout.alignment(),
                            class,
                        })
                })
                .ok_or(crate::MachineProgramError::MissingStoredLayout(ty)),
            None => Err(crate::MachineProgramError::MissingStoredLayout(ty)),
        }
    }

    fn finish(
        self,
        abi: MachineCallableAbi,
        result: Option<MachineValueId>,
    ) -> Result<MachineFunction, crate::MachineProgramError> {
        let operations = (0..self.operations.len())
            .map(MachineOperationId::new)
            .collect::<Vec<_>>();
        let mut execution = crate::MachineFunctionExecution::Immediate;
        let body = crate::MachineBody::freeze(
            crate::program::MachineBodyDraft {
                parameters: self.parameters,
                stack: self.stack,
                drop_flags: Vec::new(),
                addresses: self.addresses,
                values: self.values,
                operations: self.operations,
                packs: Vec::new(),
                blocks: vec![MachineBlock::new(
                    [],
                    operations,
                    MachineTerminator::Return(result),
                )],
                entry: MachineBlockId::new(0),
            },
            &mut execution,
        )
        .map_err(|error| crate::MachineProgramError::Optimization {
            owner: self.owner,
            error,
        })?;
        MachineFunction::new(
            self.owner,
            MachineFunctionKind::Callable(abi),
            execution,
            body,
        )
        .map_err(|error| crate::MachineProgramError::Dataflow {
            owner: self.owner,
            error,
        })
    }
}
