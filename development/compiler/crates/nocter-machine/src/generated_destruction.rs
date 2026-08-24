use nocter_model::TypeId;
use nocter_runtime_contract::{RuntimePrimitive, RuntimeType, RuntimeTypeTable};

use crate::identity::{MachineId, MachineTable};
use crate::{
    MachineAddress, MachineAddressRoot, MachineAddressStep, MachineBinaryOperation, MachineBlock,
    MachineBlockId, MachineBranchTarget, MachineCallableAbi, MachineConstant,
    MachineDestructionKind, MachineDestructionPlan, MachineFunction, MachineFunctionKind,
    MachineLayoutStore, MachineLinkageId, MachineOperation, MachineOperationId,
    MachineOperationKind, MachineResultAbi, MachineStackId, MachineStackObject,
    MachineStackPurpose, MachineSwitchCase, MachineSwitchValue, MachineTerminator, MachineValue,
    MachineValueDefinition, MachineValueId, MachineValueRepresentation,
};

/// Materializes one concrete recursive destruction plan as an ordinary machine CFG. The generated
/// function uses the compiler-owned `(byte_pointer, byte_offset)` ABI shared by pointer primitives
/// and pack callbacks, and delegates authored `drop` bodies through normal direct calls.
pub(crate) fn generate_destruction_function(
    linkage: MachineLinkageId,
    plan: &MachineDestructionPlan,
    abi: &MachineCallableAbi,
    types: &RuntimeTypeTable,
    layouts: &MachineLayoutStore,
) -> Result<MachineFunction, crate::MachineProgramError> {
    let mut builder = DestructionBuilder::new(linkage, abi, types, layouts)?;
    let pointer = builder.load_parameter(0)?;
    let offset = builder.load_parameter(1)?;
    builder.emit_plan(plan, pointer, vec![MachineAddressStep::OffsetValue(offset)])?;
    builder.finish(abi.clone())
}

struct BlockDraft {
    parameters: Vec<MachineValueId>,
    operations: Vec<MachineOperationId>,
    terminator: Option<MachineTerminator>,
}

#[derive(Clone, Copy)]
struct OutcomeEmission<'a> {
    subject: crate::MachineAddressId,
    tag_offset: u64,
    payload_offset: u64,
    active_tag: u8,
    payload: &'a MachineDestructionPlan,
}

struct DestructionBuilder<'a> {
    owner: MachineLinkageId,
    types: &'a RuntimeTypeTable,
    layouts: &'a MachineLayoutStore,
    parameters: Vec<MachineStackId>,
    stack: Vec<MachineStackObject>,
    addresses: Vec<MachineAddress>,
    values: Vec<MachineValue>,
    operations: Vec<MachineOperation>,
    blocks: Vec<BlockDraft>,
    current: MachineBlockId,
    usize_: TypeId,
    bool_: TypeId,
}

impl<'a> DestructionBuilder<'a> {
    fn new(
        owner: MachineLinkageId,
        abi: &MachineCallableAbi,
        types: &'a RuntimeTypeTable,
        layouts: &'a MachineLayoutStore,
    ) -> Result<Self, crate::MachineProgramError> {
        if abi.arguments().len() != 2 || abi.result() != MachineResultAbi::Completion {
            return Err(crate::MachineProgramError::InvalidDestructionAbi(owner));
        }
        let mut stack = Vec::with_capacity(2);
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
            blocks: vec![BlockDraft {
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: None,
            }],
            current: MachineBlockId::new(0),
            usize_: runtime_primitive(types, RuntimePrimitive::Usize)?,
            bool_: runtime_primitive(types, RuntimePrimitive::Bool)?,
        })
    }

    fn load_parameter(
        &mut self,
        position: usize,
    ) -> Result<MachineValueId, crate::MachineProgramError> {
        let stack = *self.parameters.get(position).ok_or(
            crate::MachineProgramError::InvalidDestructionAbi(self.owner),
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

    fn emit_plan(
        &mut self,
        plan: &MachineDestructionPlan,
        pointer: MachineValueId,
        steps: Vec<MachineAddressStep>,
    ) -> Result<(), crate::MachineProgramError> {
        let subject = self.plan_address(plan, pointer, steps.clone());
        match plan.kind() {
            MachineDestructionKind::Struct { drop, fields } => {
                self.emit_struct(subject, *drop, fields, pointer, &steps)?;
            }
            MachineDestructionKind::Enum {
                drop,
                tag_offset,
                variants,
            } => self.emit_enum(subject, *drop, *tag_offset, variants, pointer, &steps)?,
            MachineDestructionKind::FixedArray {
                length,
                stride,
                element,
            } => self.emit_array(element, *length, *stride, pointer, steps)?,
            MachineDestructionKind::Outcome {
                tag_offset,
                payload_offset,
                active_tag,
                payload,
            } => {
                self.emit_outcome(
                    OutcomeEmission {
                        subject,
                        tag_offset: *tag_offset,
                        payload_offset: *payload_offset,
                        active_tag: *active_tag,
                        payload,
                    },
                    pointer,
                    &steps,
                )?;
            }
            MachineDestructionKind::Fallible {
                tag_offset,
                payload_offset,
                success,
                failure,
            } => {
                if let Some(success) = success {
                    self.emit_outcome(
                        OutcomeEmission {
                            subject,
                            tag_offset: *tag_offset,
                            payload_offset: *payload_offset,
                            active_tag: crate::MachineOutcomeKind::Fallible.primary_tag(),
                            payload: success,
                        },
                        pointer,
                        &steps,
                    )?;
                }
                self.emit_outcome(
                    OutcomeEmission {
                        subject,
                        tag_offset: *tag_offset,
                        payload_offset: *payload_offset,
                        active_tag: crate::MachineOutcomeKind::Fallible.alternate_tag(),
                        payload: failure,
                    },
                    pointer,
                    &steps,
                )?;
            }
            MachineDestructionKind::Error => {
                self.append_effect(MachineOperationKind::ReleaseError { place: subject })?;
            }
            MachineDestructionKind::Closure(captures) => {
                self.emit_captures(captures, pointer, &steps)?;
            }
            MachineDestructionKind::Opaque(inner) => self.emit_plan(inner, pointer, steps)?,
        }
        Ok(())
    }

    fn emit_struct(
        &mut self,
        subject: crate::MachineAddressId,
        drop: Option<crate::MachineFunctionId>,
        fields: &[crate::MachineDestructionField],
        pointer: MachineValueId,
        steps: &[MachineAddressStep],
    ) -> Result<(), crate::MachineProgramError> {
        if let Some(drop) = drop {
            self.append_effect(MachineOperationKind::InvokeDrop {
                target: drop,
                place: subject,
                allocation: crate::MachineCallAllocation::Inherit,
            })?;
        }
        for field in fields {
            self.emit_plan(field.plan(), pointer, with_offset(steps, field.offset()))?;
        }
        Ok(())
    }

    fn emit_enum(
        &mut self,
        subject: crate::MachineAddressId,
        drop: Option<crate::MachineFunctionId>,
        tag_offset: u64,
        variants: &[crate::MachineDestructionVariant],
        pointer: MachineValueId,
        steps: &[MachineAddressStep],
    ) -> Result<(), crate::MachineProgramError> {
        if let Some(drop) = drop {
            self.append_effect(MachineOperationKind::InvokeDrop {
                target: drop,
                place: subject,
                allocation: crate::MachineCallAllocation::Inherit,
            })?;
        }
        if variants.is_empty() {
            return Ok(());
        }
        let join = self.create_block([])?;
        let mut branches = Vec::with_capacity(variants.len());
        for _ in variants {
            branches.push(self.create_block([])?);
        }
        let cases = variants
            .iter()
            .zip(&branches)
            .map(|(variant, block)| {
                MachineSwitchCase::new(
                    MachineSwitchValue::Tag(variant.tag()),
                    MachineBranchTarget::new(*block, []),
                )
            })
            .collect::<Vec<_>>();
        self.terminate(MachineTerminator::SwitchTag {
            subject,
            tag_offset,
            cases: cases.into_boxed_slice(),
            fallback: MachineBranchTarget::new(join, []),
        })?;
        for (variant, block) in variants.iter().zip(branches) {
            self.current = block;
            for payload in variant.payload() {
                self.emit_plan(
                    payload.plan(),
                    pointer,
                    with_offset(steps, payload.offset()),
                )?;
            }
            self.goto(join, [])?;
        }
        self.current = join;
        Ok(())
    }

    fn emit_outcome(
        &mut self,
        outcome: OutcomeEmission<'_>,
        pointer: MachineValueId,
        steps: &[MachineAddressStep],
    ) -> Result<(), crate::MachineProgramError> {
        let join = self.create_block([])?;
        let active = self.create_block([])?;
        self.terminate(MachineTerminator::SwitchTag {
            subject: outcome.subject,
            tag_offset: outcome.tag_offset,
            cases: vec![MachineSwitchCase::new(
                MachineSwitchValue::Tag(outcome.active_tag),
                MachineBranchTarget::new(active, []),
            )]
            .into_boxed_slice(),
            fallback: MachineBranchTarget::new(join, []),
        })?;
        self.current = active;
        self.emit_plan(
            outcome.payload,
            pointer,
            with_offset(steps, outcome.payload_offset),
        )?;
        self.goto(join, [])?;
        self.current = join;
        Ok(())
    }

    fn emit_captures(
        &mut self,
        captures: &[crate::MachineDestructionCapture],
        pointer: MachineValueId,
        steps: &[MachineAddressStep],
    ) -> Result<(), crate::MachineProgramError> {
        for capture in captures {
            self.emit_plan(
                capture.plan(),
                pointer,
                with_offset(steps, capture.offset()),
            )?;
        }
        Ok(())
    }

    fn emit_array(
        &mut self,
        element: &MachineDestructionPlan,
        length: u64,
        stride: u64,
        pointer: MachineValueId,
        steps: Vec<MachineAddressStep>,
    ) -> Result<(), crate::MachineProgramError> {
        let initial = self.integer(length)?;
        let header = self.create_block([self.usize_])?;
        let index = self.blocks[header.index()].parameters[0];
        let body = self.create_block([])?;
        let done = self.create_block([])?;
        self.goto(header, [initial])?;

        self.current = header;
        let zero = self.integer(0)?;
        let has_element = self.append_value(
            self.bool_,
            MachineOperationKind::Binary {
                operation: MachineBinaryOperation::Less,
                left: zero,
                right: index,
            },
        )?;
        self.terminate(MachineTerminator::Branch {
            condition: has_element,
            then_target: MachineBranchTarget::new(body, []),
            else_target: MachineBranchTarget::new(done, []),
        })?;

        self.current = body;
        let one = self.integer(1)?;
        let next = self.append_value(
            self.usize_,
            MachineOperationKind::Binary {
                operation: MachineBinaryOperation::Subtract,
                left: index,
                right: one,
            },
        )?;
        let stride = self.integer(stride)?;
        let offset = self.append_value(
            self.usize_,
            MachineOperationKind::Binary {
                operation: MachineBinaryOperation::Multiply,
                left: next,
                right: stride,
            },
        )?;
        let mut element_steps = steps;
        element_steps.push(MachineAddressStep::OffsetValue(offset));
        self.emit_plan(element, pointer, element_steps)?;
        self.goto(header, [next])?;
        self.current = done;
        Ok(())
    }

    fn integer(&mut self, value: u64) -> Result<MachineValueId, crate::MachineProgramError> {
        self.append_value(
            self.usize_,
            MachineOperationKind::Constant(MachineConstant::Integer(i128::from(value))),
        )
    }

    fn plan_address(
        &mut self,
        plan: &MachineDestructionPlan,
        pointer: MachineValueId,
        steps: Vec<MachineAddressStep>,
    ) -> crate::MachineAddressId {
        self.add_address(MachineAddress::new(
            plan.ty(),
            plan.size(),
            plan.alignment(),
            MachineAddressRoot::Pointer { value: pointer },
            steps,
        ))
    }

    fn add_address(&mut self, address: MachineAddress) -> crate::MachineAddressId {
        let id = crate::MachineAddressId::new(self.addresses.len());
        self.addresses.push(address);
        id
    }

    fn append_effect(
        &mut self,
        kind: MachineOperationKind,
    ) -> Result<(), crate::MachineProgramError> {
        self.append_operation(kind, None).map(|_| ())
    }

    fn append_value(
        &mut self,
        ty: TypeId,
        kind: MachineOperationKind,
    ) -> Result<MachineValueId, crate::MachineProgramError> {
        let operation = MachineOperationId::new(self.operations.len());
        let value = MachineValueId::new(self.values.len());
        let representation = self.value_representation(ty)?;
        self.values.push(MachineValue::new(
            ty,
            representation,
            MachineValueDefinition::Operation(operation),
        ));
        self.append_operation(kind, Some(value))?;
        Ok(value)
    }

    fn append_operation(
        &mut self,
        kind: MachineOperationKind,
        result: Option<MachineValueId>,
    ) -> Result<MachineOperationId, crate::MachineProgramError> {
        let operation = MachineOperationId::new(self.operations.len());
        self.operations.push(MachineOperation::new(kind, result));
        self.current_block_mut()?.operations.push(operation);
        Ok(operation)
    }

    fn create_block(
        &mut self,
        parameters: impl IntoIterator<Item = TypeId>,
    ) -> Result<MachineBlockId, crate::MachineProgramError> {
        let block = MachineBlockId::new(self.blocks.len());
        let mut values = Vec::new();
        for (position, ty) in parameters.into_iter().enumerate() {
            let value = MachineValueId::new(self.values.len());
            let representation = self.value_representation(ty)?;
            self.values.push(MachineValue::new(
                ty,
                representation,
                MachineValueDefinition::BlockParameter { block, position },
            ));
            values.push(value);
        }
        self.blocks.push(BlockDraft {
            parameters: values,
            operations: Vec::new(),
            terminator: None,
        });
        Ok(block)
    }

    fn goto(
        &mut self,
        block: MachineBlockId,
        arguments: impl Into<Box<[MachineValueId]>>,
    ) -> Result<(), crate::MachineProgramError> {
        self.terminate(MachineTerminator::Goto(MachineBranchTarget::new(
            block, arguments,
        )))
    }

    fn terminate(
        &mut self,
        terminator: MachineTerminator,
    ) -> Result<(), crate::MachineProgramError> {
        let owner = self.owner;
        let current = self.current;
        let block = self.current_block_mut()?;
        if block.terminator.replace(terminator).is_some() {
            return Err(crate::MachineProgramError::InvalidGeneratedDestruction(
                owner, current,
            ));
        }
        Ok(())
    }

    fn current_block_mut(&mut self) -> Result<&mut BlockDraft, crate::MachineProgramError> {
        self.blocks.get_mut(self.current.index()).ok_or(
            crate::MachineProgramError::InvalidGeneratedDestruction(self.owner, self.current),
        )
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
                .map(|layout| MachineValueRepresentation::Stored {
                    size: layout.size(),
                    alignment: layout.alignment(),
                })
                .ok_or(crate::MachineProgramError::MissingStoredLayout(ty)),
            None => Err(crate::MachineProgramError::MissingStoredLayout(ty)),
        }
    }

    fn finish(
        mut self,
        abi: MachineCallableAbi,
    ) -> Result<MachineFunction, crate::MachineProgramError> {
        self.terminate(MachineTerminator::Return(None))?;
        let owner = self.owner;
        let blocks = self
            .blocks
            .into_iter()
            .enumerate()
            .map(|(index, block)| {
                let id = MachineBlockId::new(index);
                block
                    .terminator
                    .map(|terminator| {
                        MachineBlock::new(block.parameters, block.operations, terminator)
                    })
                    .ok_or(crate::MachineProgramError::InvalidGeneratedDestruction(
                        owner, id,
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let body = crate::MachineBody::new(
            self.parameters,
            crate::program::MachineBodyDomains {
                stack: MachineTable::from_values(self.stack),
                drop_flags: MachineTable::from_values(Vec::<crate::MachineDropFlag>::new()),
                addresses: MachineTable::from_values(self.addresses),
                values: MachineTable::from_values(self.values),
                operations: MachineTable::from_values(self.operations),
                packs: MachineTable::from_values(Vec::<crate::MachinePack>::new()),
                blocks: MachineTable::from_values(blocks),
            },
            MachineBlockId::new(0),
        );
        MachineFunction::new(owner, MachineFunctionKind::Callable(abi), body)
            .map_err(|error| crate::MachineProgramError::Dataflow { owner, error })
    }
}

fn with_offset(steps: &[MachineAddressStep], offset: u64) -> Vec<MachineAddressStep> {
    let mut result = steps.to_vec();
    if offset != 0 {
        result.push(MachineAddressStep::Offset(offset));
    }
    result
}

fn runtime_primitive(
    types: &RuntimeTypeTable,
    primitive: RuntimePrimitive,
) -> Result<TypeId, crate::MachineProgramError> {
    types
        .primitive(primitive)
        .ok_or(crate::MachineProgramError::MissingRuntimePrimitive(
            primitive,
        ))
}
