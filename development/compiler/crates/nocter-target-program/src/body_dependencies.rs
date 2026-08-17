use std::collections::HashSet;
use std::fmt;

use nocter_checking::{
    AggregateConstruction, AllocationSelection, BorrowConversionImplementation, CallTarget,
    CheckedBody, CheckedControl, CheckedOperation, CheckedOutcome, CheckedPlace,
    CheckedReadonlyOperand, CheckedReceiver, CleanupTarget, ComparisonImplementation,
    DropSelection, InterpolationPart, IterationAcquisition, LoopKind, PlaceProjection,
    PrimitiveOperation, ReadonlyOperandPreparation, ReceiverPreparation, SequenceElement,
    StaticSelection, TypedIteration,
};
use nocter_model::{
    BodyId, BodyNodeId, BorrowCapability, ClosureId, DropId, LocalBindingId, LoopId, ParameterId,
    PlaceId, TypeId, VariantId,
};

use crate::TargetProgram;

/// The complete semantic dependency surface reachable from one checked body root.
///
/// Vectors retain deterministic first-use order while construction deduplicates identities.
/// Static selections remain unresolved here: concrete dispatch is a separate checking authority,
/// and this traversal must not guess a target callable from a requirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedBodyDependencies {
    selections: Box<[StaticSelection]>,
    closures: Box<[ClosureId]>,
    drop_selections: Box<[DropSelection]>,
    types: Box<[TypeId]>,
    prepared_borrows: Box<[PreparedBorrow]>,
    destructions: Box<[CheckedDestruction]>,
}

impl CheckedBodyDependencies {
    #[must_use]
    pub const fn selections(&self) -> &[StaticSelection] {
        &self.selections
    }

    #[must_use]
    pub const fn closures(&self) -> &[ClosureId] {
        &self.closures
    }

    #[must_use]
    pub const fn drop_selections(&self) -> &[DropSelection] {
        &self.drop_selections
    }

    /// Types used by reachable nodes, places, iteration plans, and executable cleanup actions.
    #[must_use]
    pub const fn types(&self) -> &[TypeId] {
        &self.types
    }

    /// Borrow types synthesized by operand preparation rather than represented by a checked node.
    #[must_use]
    pub const fn prepared_borrows(&self) -> &[PreparedBorrow] {
        &self.prepared_borrows
    }

    /// Exact destruction shapes required by reachable cleanup schedules.
    #[must_use]
    pub const fn destructions(&self) -> &[CheckedDestruction] {
        &self.destructions
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PreparedBorrow {
    source: TypeId,
    capability: BorrowCapability,
}

impl PreparedBorrow {
    #[must_use]
    pub const fn source(self) -> TypeId {
        self.source
    }

    #[must_use]
    pub const fn capability(self) -> BorrowCapability {
        self.capability
    }
}

/// The representation work selected by one checked cleanup target.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CheckedDestruction {
    Complete(TypeId),
    EnumResidual {
        ty: TypeId,
        variant: VariantId,
        payload: Box<[ParameterId]>,
    },
}

impl CheckedDestruction {
    #[must_use]
    pub fn for_cleanup(target: &CleanupTarget) -> Option<Self> {
        match target {
            CleanupTarget::Path(path) => Some(Self::Complete(path.ty())),
            CleanupTarget::Place { ty, .. } | CleanupTarget::Value { ty, .. } => {
                Some(Self::Complete(*ty))
            }
            CleanupTarget::EnumResidual {
                variant,
                payload,
                ty,
                ..
            } => Some(Self::EnumResidual {
                ty: *ty,
                variant: *variant,
                payload: payload.clone(),
            }),
            CleanupTarget::Region { .. } => None,
        }
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        match self {
            Self::Complete(ty) | Self::EnumResidual { ty, .. } => *ty,
        }
    }
}

/// Collects every executable semantic edge below `root`.
///
/// Source retained under [`CheckedControl::Unreachable`] and statically unreachable pattern
/// fallbacks is intentionally excluded. Cleanup targets are traversed through their exact checked
/// schedules, so type-owned drop discovery does not need to rediscover ownership flow.
///
/// # Errors
///
/// Returns a typed integrity failure when a referenced body, node, place, loop, closure, or drop
/// declaration is absent or owned by another body.
pub fn collect_body_dependencies(
    program: &TargetProgram,
    body: BodyId,
    root: BodyNodeId,
) -> Result<CheckedBodyDependencies, BodyDependencyError> {
    let checked_body = program
        .checked()
        .bodies()
        .get(body)
        .ok_or(BodyDependencyError::UnknownBody(body))?;
    let mut collector = DependencyCollector::new(program, body, checked_body);
    collector.visit_node(root)?;
    Ok(collector.finish())
}

struct DependencyCollector<'program> {
    program: &'program TargetProgram,
    body_id: BodyId,
    body: &'program CheckedBody,
    visited_nodes: HashSet<BodyNodeId>,
    visited_places: HashSet<PlaceId>,
    visited_loops: HashSet<LoopId>,
    selection_set: HashSet<StaticSelection>,
    closure_set: HashSet<ClosureId>,
    drop_set: HashSet<DropSelection>,
    type_set: HashSet<TypeId>,
    prepared_borrow_set: HashSet<PreparedBorrow>,
    destruction_set: HashSet<CheckedDestruction>,
    selections: Vec<StaticSelection>,
    closures: Vec<ClosureId>,
    drop_selections: Vec<DropSelection>,
    types: Vec<TypeId>,
    prepared_borrows: Vec<PreparedBorrow>,
    destructions: Vec<CheckedDestruction>,
}

impl<'program> DependencyCollector<'program> {
    fn new(program: &'program TargetProgram, body_id: BodyId, body: &'program CheckedBody) -> Self {
        Self {
            program,
            body_id,
            body,
            visited_nodes: HashSet::new(),
            visited_places: HashSet::new(),
            visited_loops: HashSet::new(),
            selection_set: HashSet::new(),
            closure_set: HashSet::new(),
            drop_set: HashSet::new(),
            type_set: HashSet::new(),
            prepared_borrow_set: HashSet::new(),
            destruction_set: HashSet::new(),
            selections: Vec::new(),
            closures: Vec::new(),
            drop_selections: Vec::new(),
            types: Vec::new(),
            prepared_borrows: Vec::new(),
            destructions: Vec::new(),
        }
    }

    fn finish(self) -> CheckedBodyDependencies {
        CheckedBodyDependencies {
            selections: self.selections.into_boxed_slice(),
            closures: self.closures.into_boxed_slice(),
            drop_selections: self.drop_selections.into_boxed_slice(),
            types: self.types.into_boxed_slice(),
            prepared_borrows: self.prepared_borrows.into_boxed_slice(),
            destructions: self.destructions.into_boxed_slice(),
        }
    }

    fn visit_node(&mut self, id: BodyNodeId) -> Result<(), BodyDependencyError> {
        if !self.visited_nodes.insert(id) {
            return Ok(());
        }
        let node = self
            .body
            .nodes()
            .get(id)
            .cloned()
            .ok_or(BodyDependencyError::UnknownNode {
                body: self.body_id,
                node: id,
            })?;
        if matches!(
            node.operation(),
            CheckedOperation::Control(CheckedControl::Unreachable(_))
        ) {
            return Ok(());
        }
        self.record_type(node.ty())?;
        self.visit_operation(node.operation())?;
        if let Some(schedules) = self.body.cleanups().schedules(id).map(<[_]>::to_vec) {
            for schedule in schedules {
                for action in schedule.actions() {
                    self.visit_cleanup(action.target())?;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn visit_operation(&mut self, operation: &CheckedOperation) -> Result<(), BodyDependencyError> {
        match operation {
            CheckedOperation::Complete | CheckedOperation::Constant(_) => {}
            CheckedOperation::Place(place)
            | CheckedOperation::Copy(place)
            | CheckedOperation::Move(place)
            | CheckedOperation::Borrow { place, .. } => self.visit_place(*place)?,
            CheckedOperation::Call(call) => {
                match call.target() {
                    CallTarget::Static(selection) => self.record_selection(selection),
                    CallTarget::ClosureValue { value, closure, .. } => {
                        self.visit_node(*value)?;
                        self.record_closure(*closure)?;
                    }
                    CallTarget::CallableValue {
                        value, dispatch, ..
                    } => {
                        self.visit_node(*value)?;
                        self.record_selection(dispatch);
                    }
                }
                if let Some(receiver) = call.receiver() {
                    self.visit_receiver(receiver)?;
                }
                for argument in call.arguments() {
                    self.visit_node(*argument)?;
                }
            }
            CheckedOperation::BorrowConversion(conversion) => {
                self.visit_node(conversion.value())?;
                self.record_type(conversion.target())?;
                if let BorrowConversionImplementation::Selected(selection) =
                    conversion.implementation()
                {
                    self.record_selection(selection);
                }
            }
            CheckedOperation::OpaqueWitness(witness) => {
                self.visit_node(witness.value())?;
                self.record_type(witness.witness())?;
                if self
                    .program
                    .checked()
                    .opaque_witnesses()
                    .get(witness.definition())
                    != Some(witness.witness())
                {
                    return Err(BodyDependencyError::InvalidOpaqueWitness(
                        witness.definition(),
                    ));
                }
            }
            CheckedOperation::Comparison(comparison) => {
                self.visit_readonly_operand(comparison.left())?;
                self.visit_readonly_operand(comparison.right())?;
                if let ComparisonImplementation::Selected(selection) = comparison.implementation() {
                    self.record_selection(selection);
                }
            }
            CheckedOperation::Primitive(primitive) => match primitive {
                PrimitiveOperation::Unary { operand, .. }
                | PrimitiveOperation::IntegerConversion { operand, .. } => {
                    self.visit_node(*operand)?;
                    if let PrimitiveOperation::IntegerConversion { target, .. } = primitive {
                        self.record_type(*target)?;
                    }
                }
                PrimitiveOperation::Binary { left, right, .. } => {
                    self.visit_node(*left)?;
                    self.visit_node(*right)?;
                }
            },
            CheckedOperation::Aggregate(aggregate) => match aggregate {
                AggregateConstruction::Struct { fields, .. } => {
                    for (_, value) in fields {
                        self.visit_node(*value)?;
                    }
                }
                AggregateConstruction::Enum { payload, .. }
                | AggregateConstruction::FixedArray(payload) => {
                    for value in payload {
                        self.visit_node(*value)?;
                    }
                }
            },
            CheckedOperation::Outcome(outcome) => self.visit_outcome(outcome)?,
            CheckedOperation::Closure(closure) => {
                for capture in closure.captures() {
                    self.visit_node(capture.initializer())?;
                }
                self.record_closure(closure.closure())?;
            }
            CheckedOperation::IteratorAcquisition(iteration) => {
                self.visit_receiver(iteration.source())?;
                if let IterationAcquisition::Expansion(selection) = iteration.acquisition() {
                    self.record_selection(selection);
                }
            }
            CheckedOperation::Sequence(sequence) => {
                self.record_selection(sequence.constructor());
                for element in sequence.elements() {
                    match element {
                        SequenceElement::Value(value) => self.visit_node(*value)?,
                        SequenceElement::Spread {
                            iteration,
                            exact_size,
                            ..
                        } => {
                            self.visit_iteration(iteration)?;
                            self.record_selection(exact_size);
                        }
                    }
                }
                self.visit_allocation(sequence.allocation())?;
            }
            CheckedOperation::StringLiteral {
                constructor,
                allocation,
                ..
            } => {
                self.record_selection(constructor);
                self.visit_allocation(*allocation)?;
            }
            CheckedOperation::Interpolation(interpolation) => {
                self.record_type(interpolation.output())?;
                for part in interpolation.parts() {
                    match part {
                        InterpolationPart::Text(_) => {}
                        InterpolationPart::Formatted { operand, formatter } => {
                            self.visit_readonly_operand(operand)?;
                            self.record_selection(formatter);
                        }
                        InterpolationPart::Diverging(node) => self.visit_node(*node)?,
                    }
                }
                self.visit_allocation(interpolation.allocation())?;
            }
            CheckedOperation::Control(control) => self.visit_control(control)?,
        }
        Ok(())
    }

    fn visit_receiver(&mut self, receiver: &CheckedReceiver) -> Result<(), BodyDependencyError> {
        self.visit_node(receiver.value())?;
        if let ReceiverPreparation::BorrowPlace(capability)
        | ReceiverPreparation::BorrowTemporary(capability) = receiver.preparation()
        {
            self.record_prepared_borrow(receiver.value(), capability)?;
        }
        if let Some(coercion) = receiver.coercion() {
            self.record_selection(coercion.selection());
        }
        Ok(())
    }

    fn visit_readonly_operand(
        &mut self,
        operand: &CheckedReadonlyOperand,
    ) -> Result<(), BodyDependencyError> {
        self.visit_node(operand.value())?;
        if matches!(
            operand.preparation(),
            ReadonlyOperandPreparation::BorrowPlace | ReadonlyOperandPreparation::BorrowTemporary
        ) {
            self.record_prepared_borrow(operand.value(), BorrowCapability::Readonly)?;
        } else if operand.preparation() == ReadonlyOperandPreparation::WeakenReadwriteBorrow {
            let source = self
                .body
                .nodes()
                .get(operand.value())
                .map(nocter_checking::CheckedNode::ty)
                .ok_or(BodyDependencyError::UnknownNode {
                    body: self.body_id,
                    node: operand.value(),
                })?;
            let Some(nocter_model::TypeKind::Borrow { referent, .. }) =
                self.program.checked().types().get(source)
            else {
                return Err(BodyDependencyError::UnknownType(source));
            };
            self.record_prepared_borrow_type(*referent, BorrowCapability::Readonly)?;
        }
        if let Some(coercion) = operand.coercion() {
            self.record_selection(coercion);
        }
        Ok(())
    }

    fn visit_iteration(&mut self, iteration: &TypedIteration) -> Result<(), BodyDependencyError> {
        self.visit_node(iteration.iterator())?;
        self.record_selection(iteration.next());
        self.record_type(iteration.item())
    }

    fn visit_allocation(
        &mut self,
        allocation: AllocationSelection,
    ) -> Result<(), BodyDependencyError> {
        if let AllocationSelection::Explicit(node) = allocation {
            self.visit_node(node)?;
        }
        Ok(())
    }

    fn visit_outcome(&mut self, outcome: &CheckedOutcome) -> Result<(), BodyDependencyError> {
        match outcome {
            CheckedOutcome::Absent => {}
            CheckedOutcome::Inject { payload, .. }
            | CheckedOutcome::Failure(payload)
            | CheckedOutcome::Propagate {
                operand: payload, ..
            }
            | CheckedOutcome::Force {
                operand: payload, ..
            } => self.visit_node(*payload)?,
            CheckedOutcome::Recover {
                operand,
                binding,
                fallback,
                ..
            } => {
                self.visit_node(*operand)?;
                if let Some(binding) = binding {
                    let local = self.body.locals().get(*binding).copied().ok_or(
                        BodyDependencyError::UnknownLocal {
                            body: self.body_id,
                            local: *binding,
                        },
                    )?;
                    self.record_type(local.ty())?;
                }
                self.visit_node(*fallback)?;
            }
        }
        Ok(())
    }

    fn visit_control(&mut self, control: &CheckedControl) -> Result<(), BodyDependencyError> {
        match control {
            CheckedControl::Block {
                statements, result, ..
            } => {
                for statement in statements {
                    self.visit_node(*statement)?;
                }
                if let Some(result) = result {
                    self.visit_node(*result)?;
                }
            }
            CheckedControl::Bind { initializer, .. } => self.visit_node(*initializer)?,
            CheckedControl::Assign { target, value }
            | CheckedControl::CompoundAssign { target, value, .. } => {
                self.visit_node(*value)?;
                self.visit_place(*target)?;
            }
            CheckedControl::Discard(node) => self.visit_node(*node)?,
            CheckedControl::Unreachable(_)
            | CheckedControl::Break(_)
            | CheckedControl::Continue(_) => {}
            CheckedControl::Return(value) => {
                if let Some(value) = value {
                    self.visit_node(*value)?;
                }
            }
            CheckedControl::Drop(place) => self.visit_place(*place)?,
            CheckedControl::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.visit_node(*condition)?;
                self.visit_node(*then_branch)?;
                if let Some(else_branch) = else_branch {
                    self.visit_node(*else_branch)?;
                }
            }
            CheckedControl::Logical { left, right, .. } => {
                self.visit_node(*left)?;
                self.visit_node(*right)?;
            }
            CheckedControl::Pattern {
                subject,
                arms,
                fallback,
                ..
            } => {
                self.visit_node(subject.value())?;
                for arm in arms {
                    if let Some(drop) = arm.pattern().before_transfer_drop() {
                        self.record_drop(drop)?;
                    }
                    self.visit_node(arm.body())?;
                }
                if let Some(fallback) = fallback.filter(|fallback| fallback.reachable()) {
                    self.visit_node(fallback.body())?;
                }
            }
            CheckedControl::Loop(loop_id) => self.visit_loop(*loop_id)?,
            CheckedControl::Region {
                allocator, body, ..
            } => {
                self.visit_node(*allocator)?;
                self.visit_node(*body)?;
            }
        }
        Ok(())
    }

    fn visit_loop(&mut self, id: LoopId) -> Result<(), BodyDependencyError> {
        if !self.visited_loops.insert(id) {
            return Ok(());
        }
        let loop_ = self
            .body
            .loops()
            .get(id)
            .cloned()
            .ok_or(BodyDependencyError::UnknownLoop {
                body: self.body_id,
                loop_: id,
            })?;
        match loop_.kind() {
            LoopKind::Infinite => {}
            LoopKind::While { condition } => self.visit_node(*condition)?,
            LoopKind::For { iteration, .. } => self.visit_iteration(iteration)?,
            LoopKind::Range { start, end, .. } => {
                self.visit_node(*start)?;
                self.visit_node(*end)?;
            }
        }
        self.visit_node(loop_.body())
    }

    fn visit_place(&mut self, id: PlaceId) -> Result<(), BodyDependencyError> {
        if !self.visited_places.insert(id) {
            return Ok(());
        }
        let place =
            self.body
                .places()
                .get(id)
                .cloned()
                .ok_or(BodyDependencyError::UnknownPlace {
                    body: self.body_id,
                    place: id,
                })?;
        self.record_type(place.ty())?;
        for ty in place.projection_types() {
            self.record_type(*ty)?;
        }
        self.visit_place_projections(&place)
    }

    fn visit_place_projections(&mut self, place: &CheckedPlace) -> Result<(), BodyDependencyError> {
        for projection in place.projections() {
            match projection {
                PlaceProjection::Field(_) | PlaceProjection::BorrowDeref { .. } => {}
                PlaceProjection::BuiltinIndex { index } => self.visit_node(*index)?,
                PlaceProjection::CoercedBuiltinIndex {
                    index,
                    receiver_coercion,
                } => {
                    self.visit_node(*index)?;
                    self.record_selection(receiver_coercion);
                }
                PlaceProjection::SelectedIndex {
                    index,
                    operation,
                    receiver_coercion,
                } => {
                    self.visit_node(*index)?;
                    if let Some(receiver_coercion) = receiver_coercion {
                        self.record_selection(receiver_coercion);
                    }
                    self.record_selection(operation);
                }
            }
        }
        Ok(())
    }

    fn visit_cleanup(&mut self, target: &CleanupTarget) -> Result<(), BodyDependencyError> {
        if let Some(destruction) = CheckedDestruction::for_cleanup(target) {
            self.record_destruction(destruction)?;
        }
        match target {
            CleanupTarget::Path(path) => {
                for ty in path.projection_types() {
                    self.record_type(*ty)?;
                }
            }
            CleanupTarget::Place { place, ty } => {
                self.visit_place(*place)?;
                self.record_type(*ty)?;
            }
            CleanupTarget::Value { node, ty } => {
                self.visit_node(*node)?;
                self.record_type(*ty)?;
            }
            CleanupTarget::EnumResidual { subject, ty, .. } => {
                self.visit_node(*subject)?;
                self.record_type(*ty)?;
            }
            CleanupTarget::Region { parent, .. } => self.visit_node(*parent)?,
        }
        Ok(())
    }

    fn record_selection(&mut self, selection: &StaticSelection) {
        if self.selection_set.insert(selection.clone()) {
            self.selections.push(selection.clone());
        }
    }

    fn record_closure(&mut self, closure: ClosureId) -> Result<(), BodyDependencyError> {
        let definition = self
            .program
            .checked()
            .closures()
            .get(closure)
            .ok_or(BodyDependencyError::UnknownClosure(closure))?;
        if definition.owner() != self.body_id {
            return Err(BodyDependencyError::ClosureOwnerMismatch {
                closure,
                expected: self.body_id,
                actual: definition.owner(),
            });
        }
        if self.closure_set.insert(closure) {
            self.closures.push(closure);
        }
        Ok(())
    }

    fn record_drop(&mut self, selection: &DropSelection) -> Result<(), BodyDependencyError> {
        let declaration = self
            .program
            .checked()
            .graph()
            .declarations()
            .drops()
            .get(selection.declaration())
            .ok_or(BodyDependencyError::UnknownDrop(selection.declaration()))?;
        if declaration.generic_parameters().len() != selection.generic_arguments().as_slice().len()
            || declaration
                .generic_parameters()
                .iter()
                .any(|parameter| selection.generic_arguments().get(*parameter).is_none())
        {
            return Err(BodyDependencyError::InvalidDropArguments(
                selection.declaration(),
            ));
        }
        for argument in selection.generic_arguments().as_slice() {
            self.record_type(argument.ty())?;
        }
        if self.drop_set.insert(selection.clone()) {
            self.drop_selections.push(selection.clone());
        }
        Ok(())
    }

    fn record_type(&mut self, ty: TypeId) -> Result<(), BodyDependencyError> {
        if self.program.checked().types().get(ty).is_none() {
            return Err(BodyDependencyError::UnknownType(ty));
        }
        if self.type_set.insert(ty) {
            self.types.push(ty);
        }
        Ok(())
    }

    fn record_prepared_borrow(
        &mut self,
        node: BodyNodeId,
        capability: BorrowCapability,
    ) -> Result<(), BodyDependencyError> {
        let source = self
            .body
            .nodes()
            .get(node)
            .map(nocter_checking::CheckedNode::ty)
            .ok_or(BodyDependencyError::UnknownNode {
                body: self.body_id,
                node,
            })?;
        self.record_prepared_borrow_type(source, capability)
    }

    fn record_prepared_borrow_type(
        &mut self,
        source: TypeId,
        capability: BorrowCapability,
    ) -> Result<(), BodyDependencyError> {
        self.record_type(source)?;
        let borrow = PreparedBorrow { source, capability };
        if self.prepared_borrow_set.insert(borrow) {
            self.prepared_borrows.push(borrow);
        }
        Ok(())
    }

    fn record_destruction(
        &mut self,
        destruction: CheckedDestruction,
    ) -> Result<(), BodyDependencyError> {
        self.record_type(destruction.ty())?;
        if self.destruction_set.insert(destruction.clone()) {
            self.destructions.push(destruction);
        }
        Ok(())
    }
}

/// Checked-body integrity failure discovered while enumerating executable dependencies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyDependencyError {
    UnknownBody(BodyId),
    UnknownNode {
        body: BodyId,
        node: BodyNodeId,
    },
    UnknownPlace {
        body: BodyId,
        place: PlaceId,
    },
    UnknownLoop {
        body: BodyId,
        loop_: LoopId,
    },
    UnknownLocal {
        body: BodyId,
        local: LocalBindingId,
    },
    UnknownClosure(ClosureId),
    ClosureOwnerMismatch {
        closure: ClosureId,
        expected: BodyId,
        actual: BodyId,
    },
    UnknownDrop(DropId),
    InvalidDropArguments(DropId),
    InvalidOpaqueWitness(nocter_model::OpaqueTypeId),
    UnknownType(TypeId),
}

impl fmt::Display for BodyDependencyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "checked-body dependency invariant failed: {self:?}"
        )
    }
}

impl std::error::Error for BodyDependencyError {}
