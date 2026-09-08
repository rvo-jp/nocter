use std::collections::{BTreeMap, BTreeSet};

use nocter_checking::{
    AggregateConstruction, CheckedBody, CheckedOperation, ConstantValue, LocalBindingKind,
    PrimitiveBinary, PrimitiveOperation, PrimitiveUnary,
};
use nocter_model::{
    BodyNodeId, BuiltinType, ClosureId, ExecutableItemId, LocalBindingId, LoopId, MirBlockId,
    MirDropFlagId, MirLocalId, MirPlaceId, MirValueId, ParameterId, TypeId, TypeKind,
};
use nocter_target_program::{
    ExecutableExecution, ExecutableInputSource, ExecutableItem, ExecutableProgram,
};

use super::MirLoweringError;
use super::cleanup_flags::CleanupIdentity;
use super::loop_control::LoopTargets;
use crate::{
    MirAggregate, MirBinaryOperation, MirConstant, MirFunction, MirFunctionBuilder, MirLocalKind,
    MirOperationKind, MirPlaceRoot, MirTerminator, MirUnaryOperation,
};

pub(super) fn lower_function(
    executable: &ExecutableProgram,
    item_id: ExecutableItemId,
    item: &ExecutableItem,
) -> Result<MirFunction, MirLoweringError> {
    let checked = executable
        .checked_body(item.body().body())
        .ok_or(MirLoweringError::UnknownBody(item.body().body()))?;
    let mut lowerer = FunctionLowerer::new(executable, item_id, item, checked)?;
    lowerer.prepare_cleanup_flags()?;
    let initial_cancellation = match item.execution() {
        ExecutableExecution::Immediate => Box::new([]),
        ExecutableExecution::Deferred { .. } => lowerer.lower_initial_cancellation_actions()?,
    };
    let completed_destruction = item
        .completion_destruction()
        .map(|plan| lowerer.lower_deferred_destruction(item.body().root(), plan))
        .transpose()?;
    let result = lowerer.lower_node(item.body().root())?;
    if let Some(block) = lowerer.current {
        lowerer.destroy_pack()?;
        let body_result = match item.execution() {
            ExecutableExecution::Immediate => item.signature().result(),
            ExecutableExecution::Deferred { output } => output,
        };
        match executable.types().get(body_result) {
            Some(TypeKind::Builtin(BuiltinType::Void)) => {
                lowerer
                    .builder
                    .terminate(block, MirTerminator::Return(None))?;
            }
            Some(TypeKind::Builtin(BuiltinType::Never)) => {
                return Err(MirLoweringError::InvalidTerminalResult(item_id));
            }
            Some(_) => {
                let value = result.ok_or(MirLoweringError::MissingValue(item.body().root()))?;
                lowerer
                    .builder
                    .terminate(block, MirTerminator::Return(Some(value)))?;
            }
            None => {
                return Err(MirLoweringError::MissingConcreteType(
                    item.signature().result(),
                ));
            }
        }
    }
    let cancellation = std::mem::take(&mut lowerer.cancellation);
    let stable_storage = std::mem::take(&mut lowerer.stable_storage);
    match item.execution() {
        ExecutableExecution::Immediate => lowerer.builder.finish(lowerer.entry),
        ExecutableExecution::Deferred { .. } => lowerer.builder.finish_deferred(
            lowerer.entry,
            item.signature().result(),
            initial_cancellation,
            cancellation,
            stable_storage,
            completed_destruction,
        ),
    }
    .map_err(Into::into)
}

pub(super) struct FunctionLowerer<'a> {
    pub(super) executable: &'a ExecutableProgram,
    pub(super) item: &'a ExecutableItem,
    pub(super) body: &'a CheckedBody,
    pub(super) builder: MirFunctionBuilder,
    pub(super) entry: MirBlockId,
    pub(super) current: Option<MirBlockId>,
    pub(super) parameters: BTreeMap<ParameterId, MirLocalId>,
    pub(super) locals: BTreeMap<LocalBindingId, MirLocalId>,
    pub(super) closure_environments: BTreeMap<ClosureId, MirLocalId>,
    pub(super) values: BTreeMap<BodyNodeId, MirValueId>,
    pub(super) places: BTreeMap<nocter_model::PlaceId, MirPlaceId>,
    pub(super) value_storage: BTreeMap<BodyNodeId, MirPlaceId>,
    pub(super) materialized_value_storage: BTreeSet<BodyNodeId>,
    pub(super) cleanup_flags: BTreeMap<CleanupIdentity, MirDropFlagId>,
    pub(super) cancellation: BTreeMap<MirBlockId, Box<[crate::MirCancellationAction]>>,
    pub(super) stable_storage: BTreeMap<MirBlockId, Box<[MirLocalId]>>,
    pub(super) loops: BTreeMap<LoopId, LoopTargets>,
    /// Innermost last. These compiler-owned resources select calls without mutating ambient state.
    pub(super) regions: Vec<MirLocalId>,
}

impl<'a> FunctionLowerer<'a> {
    fn new(
        executable: &'a ExecutableProgram,
        item_id: ExecutableItemId,
        item: &'a ExecutableItem,
        body: &'a CheckedBody,
    ) -> Result<Self, MirLoweringError> {
        let body_result = match item.execution() {
            ExecutableExecution::Immediate => item.signature().result(),
            ExecutableExecution::Deferred { output } => output,
        };
        let mut builder = MirFunctionBuilder::new(item_id, body_result);
        if let Some(pack) = item.signature().pack() {
            builder.set_pack_input(crate::MirPackInput::new(pack.element(), pack.next()))?;
        }
        let mut parameters = BTreeMap::new();
        let mut locals = BTreeMap::new();
        let mut closure_environments = BTreeMap::new();
        for input in item.signature().inputs().iter().copied() {
            let local = builder.add_parameter(input.ty(), false);
            match input.source() {
                ExecutableInputSource::Parameter(parameter) => {
                    parameters.insert(parameter, local);
                }
                ExecutableInputSource::ClosureParameter(binding) => {
                    locals.insert(binding, local);
                }
                ExecutableInputSource::ClosureEnvironment(closure) => {
                    closure_environments.insert(closure, local);
                }
            }
        }
        let (entry, _) = builder.create_block([]);
        Ok(Self {
            executable,
            item,
            body,
            builder,
            entry,
            current: Some(entry),
            parameters,
            locals,
            closure_environments,
            values: BTreeMap::new(),
            places: BTreeMap::new(),
            value_storage: BTreeMap::new(),
            materialized_value_storage: BTreeSet::new(),
            cleanup_flags: BTreeMap::new(),
            cancellation: BTreeMap::new(),
            stable_storage: BTreeMap::new(),
            loops: BTreeMap::new(),
            regions: Vec::new(),
        })
    }

    pub(super) fn lower_node(
        &mut self,
        node: BodyNodeId,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        if self.current.is_none() {
            return Ok(None);
        }
        if let Some(value) = self.values.get(&node).copied() {
            return Ok(Some(value));
        }
        let checked = self
            .body
            .nodes()
            .get(node)
            .cloned()
            .ok_or(MirLoweringError::UnknownNode(node))?;
        let ty = self.concrete_type(checked.ty())?;
        let lowered = match checked.operation() {
            CheckedOperation::Complete => Ok(None),
            CheckedOperation::Constant(constant) => self.lower_constant(ty, constant).map(Some),
            CheckedOperation::Copy(place) => {
                let place = self.lower_place(*place)?;
                self.append_value(
                    ty,
                    MirOperationKind::Read {
                        place,
                        mode: crate::MirReadMode::Copy,
                    },
                )
                .map(Some)
            }
            CheckedOperation::Move(place) => {
                let checked_place = *place;
                let place = self.lower_place(checked_place)?;
                let value = self.append_value(
                    ty,
                    MirOperationKind::Read {
                        place,
                        mode: crate::MirReadMode::Move,
                    },
                )?;
                self.mark_place_initialized(checked_place, false)?;
                Ok(Some(value))
            }
            CheckedOperation::Borrow { capability, place } => {
                let place = self.lower_place(*place)?;
                self.append_value(
                    ty,
                    MirOperationKind::Borrow {
                        place,
                        capability: *capability,
                    },
                )
                .map(Some)
            }
            CheckedOperation::Await(await_) => self.lower_await(node, ty, *await_).map(Some),
            CheckedOperation::Place(_) => Err(MirLoweringError::UnsupportedOperation(node)),
            CheckedOperation::BorrowConversion(conversion) => {
                self.lower_borrow_conversion(node, conversion).map(Some)
            }
            CheckedOperation::CallableGuaranteeErasure(value) => {
                self.require_value(*value).map(Some)
            }
            CheckedOperation::OpaqueWitness(witness) => {
                self.lower_opaque_witness(node, ty, *witness).map(Some)
            }
            CheckedOperation::ArgumentPackLength(parameter) => {
                self.lower_pack_length(node, ty, *parameter).map(Some)
            }
            CheckedOperation::Comparison(comparison) => self.lower_comparison(node, comparison),
            CheckedOperation::Outcome(outcome) => self.lower_outcome(node, ty, outcome),
            CheckedOperation::Primitive(primitive) => self.lower_primitive(ty, primitive).map(Some),
            CheckedOperation::Aggregate(aggregate) => self.lower_aggregate(ty, aggregate).map(Some),
            CheckedOperation::Call(call) => self.lower_call(node, ty, call).map(Some),
            CheckedOperation::Closure(closure) => self.lower_closure(node, ty, closure).map(Some),
            CheckedOperation::StringLiteral {
                constructor,
                text,
                allocation,
            } => self
                .lower_typed_string(node, ty, constructor, text, *allocation)
                .map(Some),
            CheckedOperation::IteratorAcquisition(acquisition) => self
                .lower_iterator_acquisition(node, ty, acquisition)
                .map(Some),
            CheckedOperation::Interpolation(interpolation) => {
                self.lower_interpolation(node, ty, interpolation)
            }
            CheckedOperation::Control(control) => self.lower_control(node, control),
            CheckedOperation::PackLiteral(_) => self.lower_pack_literal(node, ty).map(Some),
        }?;
        if let Some(value) = lowered
            && self.values.insert(node, value).is_some()
        {
            return Err(MirLoweringError::InvalidDispatch(node));
        }
        if self.current.is_some() {
            self.lower_cleanup(node, nocter_checking::CleanupTiming::AtControlHeaderEnd)?;
            self.lower_cleanup(node, nocter_checking::CleanupTiming::AtStatementEnd)?;
        }
        Ok(lowered)
    }

    fn lower_constant(
        &mut self,
        ty: TypeId,
        constant: &ConstantValue,
    ) -> Result<MirValueId, MirLoweringError> {
        let constant = match constant {
            ConstantValue::Bool(value) => MirConstant::Bool(*value),
            ConstantValue::Character(value) => MirConstant::Character(*value),
            ConstantValue::Float32(bits) => MirConstant::Float32(*bits),
            ConstantValue::Float64(bits) => MirConstant::Float64(*bits),
            ConstantValue::Integer(value) => MirConstant::Integer(*value),
            ConstantValue::Text(value) => MirConstant::Text(value.clone()),
        };
        self.append_value(ty, MirOperationKind::Constant(constant))
    }

    fn lower_primitive(
        &mut self,
        ty: TypeId,
        primitive: &PrimitiveOperation,
    ) -> Result<MirValueId, MirLoweringError> {
        let kind = match primitive {
            PrimitiveOperation::Unary { operation, operand } => MirOperationKind::Unary {
                operation: match operation {
                    PrimitiveUnary::LogicalNot => MirUnaryOperation::LogicalNot,
                    PrimitiveUnary::Negate => MirUnaryOperation::Negate,
                },
                operand: self.require_value(*operand)?,
            },
            PrimitiveOperation::Binary {
                operation,
                left,
                right,
            } => MirOperationKind::Binary {
                operation: mir_binary_operation(*operation),
                left: self.require_value(*left)?,
                right: self.require_value(*right)?,
            },
            PrimitiveOperation::NumericConversion { operand, .. } => {
                MirOperationKind::NumericConversion {
                    operand: self.require_value(*operand)?,
                }
            }
        };
        self.append_value(ty, kind)
    }

    fn lower_aggregate(
        &mut self,
        ty: TypeId,
        aggregate: &AggregateConstruction,
    ) -> Result<MirValueId, MirLoweringError> {
        let aggregate = match aggregate {
            AggregateConstruction::Struct { definition, fields } => MirAggregate::Struct {
                definition: *definition,
                fields: fields
                    .iter()
                    .map(|(field, value)| Ok((*field, self.require_value(*value)?)))
                    .collect::<Result<Vec<_>, MirLoweringError>>()?
                    .into_boxed_slice(),
            },
            AggregateConstruction::Enum { variant, payload } => MirAggregate::Enum {
                variant: *variant,
                payload: payload
                    .iter()
                    .map(|value| self.require_value(*value))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            },
            AggregateConstruction::FixedArray(values) => MirAggregate::FixedArray(
                values
                    .iter()
                    .map(|value| self.require_value(*value))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            AggregateConstruction::Tuple(values) => MirAggregate::Tuple(
                values
                    .iter()
                    .map(|value| self.require_value(*value))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
        };
        self.append_value(ty, MirOperationKind::Aggregate(aggregate))
    }

    pub(super) fn require_value(
        &mut self,
        node: BodyNodeId,
    ) -> Result<MirValueId, MirLoweringError> {
        self.lower_node(node)?
            .ok_or(MirLoweringError::MissingValue(node))
    }

    pub(super) fn append_value(
        &mut self,
        ty: TypeId,
        kind: MirOperationKind,
    ) -> Result<MirValueId, MirLoweringError> {
        let block = self.current.ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.builder
            .append_value(block, ty, kind)
            .map_err(Into::into)
    }

    pub(super) fn append_effect(&mut self, kind: MirOperationKind) -> Result<(), MirLoweringError> {
        let block = self.current.ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.builder.append_effect(block, kind)?;
        Ok(())
    }

    pub(super) fn concrete_type(&self, source: TypeId) -> Result<TypeId, MirLoweringError> {
        self.item
            .body()
            .concrete_type(source)
            .ok_or(MirLoweringError::MissingConcreteType(source))
    }

    pub(super) fn ensure_local(
        &mut self,
        binding: LocalBindingId,
    ) -> Result<MirLocalId, MirLoweringError> {
        if let Some(local) = self.locals.get(&binding) {
            return Ok(*local);
        }
        let checked = self
            .body
            .locals()
            .get(binding)
            .copied()
            .ok_or(MirLoweringError::UnknownLocal(binding))?;
        let kind = match checked.declaration().kind() {
            LocalBindingKind::Region => MirLocalKind::Region,
            LocalBindingKind::Immutable
            | LocalBindingKind::Mutable
            | LocalBindingKind::PatternPayload
            | LocalBindingKind::Loop
            | LocalBindingKind::Catch
            | LocalBindingKind::ClosureParameter => MirLocalKind::User,
        };
        let mutable = checked.declaration().kind() == LocalBindingKind::Mutable;
        let ty = self.concrete_type(checked.ty())?;
        let local = self.builder.add_local(ty, kind, mutable);
        self.locals.insert(binding, local);
        Ok(local)
    }
}

impl FunctionLowerer<'_> {
    fn lower_await(
        &mut self,
        node: BodyNodeId,
        output: TypeId,
        await_: nocter_checking::CheckedAwait,
    ) -> Result<MirValueId, MirLoweringError> {
        let computation = self.require_value(await_.computation())?;
        let block = self.current.ok_or(MirLoweringError::MissingCurrentBlock)?;
        let (resume, parameters) = self.builder.create_block([output]);
        let result = parameters[0];
        let cancellation = self.lower_cancellation_actions(node, await_.computation())?;
        if self.cancellation.insert(block, cancellation).is_some() {
            return Err(MirLoweringError::InvalidCleanup(node));
        }
        let stable = self.lower_suspension_storage(node)?;
        if self
            .stable_storage
            .insert(block, stable.into_boxed_slice())
            .is_some()
        {
            return Err(MirLoweringError::InvalidSuspensionStorage(node));
        }
        self.builder.terminate(
            block,
            MirTerminator::Suspend {
                computation,
                resume: crate::MirBranchTarget::new(resume, []),
            },
        )?;
        self.current = Some(resume);
        Ok(result)
    }

    fn lower_suspension_storage(
        &mut self,
        await_node: BodyNodeId,
    ) -> Result<Vec<MirLocalId>, MirLoweringError> {
        let storage = self
            .item
            .body()
            .suspension_storage(await_node)
            .ok_or(MirLoweringError::MissingSuspensionStorage(await_node))?
            .to_vec();
        let mut locals = BTreeSet::new();
        for storage in storage {
            let local = match storage {
                nocter_target_program::ExecutableStorageIdentity::Parameter(parameter) => *self
                    .parameters
                    .get(&parameter)
                    .ok_or(MirLoweringError::MissingInput(parameter))?,
                nocter_target_program::ExecutableStorageIdentity::Local(local) => {
                    self.ensure_local(local)?
                }
                nocter_target_program::ExecutableStorageIdentity::Capture(capture) => {
                    let path = self.lower_capture_path(capture)?;
                    let MirPlaceRoot::Local(local) = path.root else {
                        return Err(MirLoweringError::InvalidSuspensionStorage(await_node));
                    };
                    local
                }
                nocter_target_program::ExecutableStorageIdentity::Value(value) => {
                    let place = self
                        .value_storage
                        .get(&value)
                        .copied()
                        .ok_or(MirLoweringError::InvalidSuspensionStorage(await_node))?;
                    let Some(MirPlaceRoot::Local(local)) =
                        self.builder.place(place).map(crate::MirPlace::root)
                    else {
                        return Err(MirLoweringError::InvalidSuspensionStorage(await_node));
                    };
                    local
                }
            };
            locals.insert(local);
        }
        Ok(locals.into_iter().collect())
    }
}

pub(super) const fn mir_binary_operation(operation: PrimitiveBinary) -> MirBinaryOperation {
    match operation {
        PrimitiveBinary::Add => MirBinaryOperation::Add,
        PrimitiveBinary::Subtract => MirBinaryOperation::Subtract,
        PrimitiveBinary::Multiply => MirBinaryOperation::Multiply,
        PrimitiveBinary::Divide => MirBinaryOperation::Divide,
        PrimitiveBinary::Remainder => MirBinaryOperation::Remainder,
        PrimitiveBinary::ShiftLeft => MirBinaryOperation::ShiftLeft,
        PrimitiveBinary::ShiftRightSigned => MirBinaryOperation::ShiftRightSigned,
        PrimitiveBinary::ShiftRightUnsigned => MirBinaryOperation::ShiftRightUnsigned,
    }
}
