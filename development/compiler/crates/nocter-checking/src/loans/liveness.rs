use std::collections::{BTreeSet, HashMap};

use nocter_model::{BodyNodeId, FieldId, LoopId};

use crate::{
    AggregateConstruction, CheckedBody, CheckedControl, CheckedOperation, CheckedOutcome,
    CleanupTarget, DropTable, LoopKind, PlaceProjection, PlaceRoot, PrimitiveOperation,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct LivePlace {
    root: PlaceRoot,
    fields: Box<[FieldId]>,
}

impl LivePlace {
    pub(super) fn from_parts(root: PlaceRoot, fields: Box<[FieldId]>) -> Self {
        Self { root, fields }
    }

    pub(super) fn from_checked(place: &crate::CheckedPlace) -> Self {
        let fields = place
            .projections()
            .iter()
            .take_while(|projection| matches!(projection, PlaceProjection::Field(_)))
            .filter_map(|projection| match projection {
                PlaceProjection::Field(field) => Some(*field),
                _ => None,
            })
            .collect();
        Self {
            root: place.root(),
            fields,
        }
    }

    pub(super) const fn root(&self) -> PlaceRoot {
        self.root
    }

    pub(super) const fn fields(&self) -> &[FieldId] {
        &self.fields
    }

    fn is_at_or_below(&self, another: &Self) -> bool {
        self.root == another.root
            && self.fields.len() >= another.fields.len()
            && self.fields.starts_with(&another.fields)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum LiveSlot {
    Node(BodyNodeId),
    Place(LivePlace),
}

pub(super) type LiveSet = BTreeSet<LiveSlot>;

pub(super) struct Liveness {
    before: HashMap<BodyNodeId, LiveSet>,
}

impl Liveness {
    pub(super) fn before(&self, node: BodyNodeId) -> Option<&LiveSet> {
        self.before.get(&node)
    }
}

struct LoopTargets {
    id: LoopId,
    break_live: LiveSet,
    continue_live: LiveSet,
}

pub(super) fn analyze(
    types: &nocter_model::TypeStore,
    drops: &DropTable,
    body: &CheckedBody,
    root: BodyNodeId,
) -> Result<Liveness, crate::BodyCheckInternalError> {
    let mut analyzer = Analyzer {
        types,
        drops,
        body,
        before: HashMap::new(),
        loops: Vec::new(),
    };
    analyzer.transfer(root, LiveSet::new())?;
    Ok(Liveness {
        before: analyzer.before,
    })
}

struct Analyzer<'body> {
    types: &'body nocter_model::TypeStore,
    drops: &'body DropTable,
    body: &'body CheckedBody,
    before: HashMap<BodyNodeId, LiveSet>,
    loops: Vec<LoopTargets>,
}

impl Analyzer<'_> {
    fn transfer(
        &mut self,
        node: BodyNodeId,
        mut live: LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        let checked = self
            .body
            .nodes()
            .get(node)
            .ok_or(crate::BodyCheckInternalError::MissingNode(node))?;
        let result_live = live.remove(&LiveSlot::Node(node));
        let cleanup_live = self.cleanup_live(node)?;
        live.extend(cleanup_live.iter().cloned());
        let operation = checked.operation().clone();
        live = match operation {
            CheckedOperation::Complete | CheckedOperation::Constant(_) => live,
            CheckedOperation::Place(place)
            | CheckedOperation::Copy(place)
            | CheckedOperation::Move(place)
            | CheckedOperation::Borrow { place, .. } => self.place_event(place, live)?,
            CheckedOperation::BorrowConversion(conversion) => {
                self.operand(conversion.value(), live)?
            }
            CheckedOperation::Primitive(operation) => self.primitive(&operation, live)?,
            CheckedOperation::Comparison(comparison) => self.operands(
                [comparison.left().value(), comparison.right().value()],
                live,
            )?,
            CheckedOperation::Call(call) => {
                let mut operands = Vec::new();
                match call.target() {
                    crate::CallTarget::CallableValue { value, .. }
                    | crate::CallTarget::ClosureValue { value, .. } => operands.push(*value),
                    crate::CallTarget::Static(_) => {}
                }
                if let Some(receiver) = call.receiver() {
                    operands.push(receiver.value());
                }
                operands.extend_from_slice(call.arguments());
                self.operands(operands, live)?
            }
            CheckedOperation::Aggregate(aggregate) => {
                let operands = match aggregate {
                    AggregateConstruction::Struct { fields, .. } => {
                        fields.iter().map(|(_, value)| *value).collect()
                    }
                    AggregateConstruction::Enum { payload, .. }
                    | AggregateConstruction::FixedArray(payload) => payload.into_vec(),
                };
                self.operands(operands, live)?
            }
            CheckedOperation::Outcome(outcome) => self.outcome(&outcome, result_live, live)?,
            CheckedOperation::Control(control) => {
                self.control(node, &control, result_live, live, &cleanup_live)?
            }
            CheckedOperation::StringLiteral { allocation, .. } => {
                self.allocation(allocation, live)?
            }
            CheckedOperation::Sequence(sequence) => {
                let mut operands = Vec::new();
                for element in sequence.elements() {
                    match element {
                        crate::SequenceElement::Value(value) => operands.push(*value),
                        crate::SequenceElement::Spread { iteration, .. } => {
                            operands.push(iteration.source());
                        }
                    }
                }
                let live = self.operands(operands, live)?;
                self.allocation(sequence.allocation(), live)?
            }
            CheckedOperation::Interpolation(interpolation) => {
                let operands = interpolation.parts().iter().filter_map(|part| match part {
                    crate::InterpolationPart::Text(_) => None,
                    crate::InterpolationPart::Formatted { value, .. } => Some(*value),
                });
                let live = self.operands(operands, live)?;
                self.allocation(interpolation.allocation(), live)?
            }
            CheckedOperation::Closure(closure) => self.operands(
                closure
                    .captures()
                    .iter()
                    .map(|capture| capture.initializer()),
                live,
            )?,
        };
        self.before.entry(node).or_default().extend(live.clone());
        Ok(live)
    }

    fn operand(
        &mut self,
        operand: BodyNodeId,
        mut live: LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        live.insert(LiveSlot::Node(operand));
        self.transfer(operand, live)
    }

    fn operands(
        &mut self,
        operands: impl IntoIterator<Item = BodyNodeId>,
        mut live: LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        let operands = operands.into_iter().collect::<Vec<_>>();
        live.extend(operands.iter().copied().map(LiveSlot::Node));
        for operand in operands.into_iter().rev() {
            live = self.transfer(operand, live)?;
        }
        Ok(live)
    }

    fn place_event(
        &mut self,
        place: nocter_model::PlaceId,
        mut live: LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        let place = self
            .body
            .places()
            .get(place)
            .ok_or(crate::BodyCheckInternalError::InvalidMovePlace(place))?;
        live.insert(LiveSlot::Place(LivePlace::from_checked(place)));
        self.operands(place.evaluation_nodes(), live)
    }

    fn primitive(
        &mut self,
        operation: &PrimitiveOperation,
        live: LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        match operation {
            PrimitiveOperation::Unary { operand, .. }
            | PrimitiveOperation::IntegerConversion { operand, .. } => self.operand(*operand, live),
            PrimitiveOperation::Binary { left, right, .. } => self.operands([*left, *right], live),
        }
    }

    fn outcome(
        &mut self,
        outcome: &CheckedOutcome,
        result_live: bool,
        live: LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        match outcome {
            CheckedOutcome::Inject { payload, .. }
            | CheckedOutcome::Failure(payload)
            | CheckedOutcome::Propagate {
                operand: payload, ..
            }
            | CheckedOutcome::Force {
                operand: payload, ..
            } => self.operand(*payload, live),
            CheckedOutcome::Absent => Ok(live),
            CheckedOutcome::Recover {
                operand,
                binding,
                fallback,
                ..
            } => {
                let mut fallback_live = live.clone();
                if result_live {
                    fallback_live.insert(LiveSlot::Node(*fallback));
                }
                fallback_live = self.transfer(*fallback, fallback_live)?;
                if let Some(binding) = binding {
                    Self::kill_root(&mut fallback_live, PlaceRoot::Local(*binding));
                }
                let mut joined = live;
                joined.extend(fallback_live);
                self.operand(*operand, joined)
            }
        }
    }

    fn control(
        &mut self,
        _node: BodyNodeId,
        control: &CheckedControl,
        result_live: bool,
        live: LiveSet,
        cleanup_live: &LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        match control {
            CheckedControl::Block {
                scope,
                statements,
                result,
            } => self.control_block(*scope, statements, *result, result_live, live),
            CheckedControl::Bind {
                binding,
                initializer,
            } => {
                let mut live = live;
                Self::kill_root(&mut live, PlaceRoot::Local(*binding));
                self.operand(*initializer, live)
            }
            CheckedControl::Assign { target, value } => {
                let mut live = live;
                self.kill_place(&mut live, *target)?;
                let place = self
                    .body
                    .places()
                    .get(*target)
                    .ok_or(crate::BodyCheckInternalError::InvalidMovePlace(*target))?;
                let mut operands = vec![*value];
                operands.extend(place.evaluation_nodes());
                self.operands(operands, live)
            }
            CheckedControl::CompoundAssign { target, value, .. } => {
                let live = self.place_event(*target, live)?;
                self.operand(*value, live)
            }
            CheckedControl::Discard(value) => self.operand(*value, live),
            CheckedControl::Unreachable(_) => Ok(live),
            CheckedControl::Return(value) => value
                .map(|value| self.operand(value, cleanup_live.clone()))
                .transpose()
                .map(|value| value.unwrap_or_else(|| cleanup_live.clone())),
            CheckedControl::Break(loop_) => {
                let mut live = self.loop_target(*loop_)?.break_live.clone();
                live.extend(cleanup_live.iter().cloned());
                Ok(live)
            }
            CheckedControl::Continue(loop_) => {
                let mut live = self.loop_target(*loop_)?.continue_live.clone();
                live.extend(cleanup_live.iter().cloned());
                Ok(live)
            }
            CheckedControl::Drop(place) => self.place_event(*place, live),
            CheckedControl::If {
                condition,
                then_branch,
                else_branch,
            } => self.control_if(*condition, *then_branch, *else_branch, result_live, live),
            CheckedControl::Logical { left, right, .. } => {
                let right_live = self.operand(*right, live.clone())?;
                let mut joined = live;
                joined.extend(right_live);
                self.operand(*left, joined)
            }
            CheckedControl::Pattern {
                subject,
                arms,
                fallback,
                ..
            } => self.control_pattern(*subject, arms, *fallback, result_live, live),
            CheckedControl::Loop(loop_) => self.loop_control(*loop_, &live),
            CheckedControl::Region {
                binding,
                allocator,
                body,
            } => {
                let mut body_live = live;
                if result_live {
                    body_live.insert(LiveSlot::Node(*body));
                }
                body_live = self.transfer(*body, body_live)?;
                Self::kill_root(&mut body_live, PlaceRoot::Local(*binding));
                self.operand(*allocator, body_live)
            }
        }
    }

    fn control_block(
        &mut self,
        scope: nocter_model::BodyScopeId,
        statements: &[BodyNodeId],
        result: Option<BodyNodeId>,
        result_live: bool,
        mut live: LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        if let Some(result) = result {
            if result_live {
                live.insert(LiveSlot::Node(result));
            }
            live = self.transfer(result, live)?;
        }
        for statement in statements.iter().rev() {
            live = self.transfer(*statement, live)?;
        }
        self.kill_scope(&mut live, scope);
        Ok(live)
    }

    fn control_if(
        &mut self,
        condition: BodyNodeId,
        then_branch: BodyNodeId,
        else_branch: Option<BodyNodeId>,
        result_live: bool,
        live: LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        let mut then_live = live.clone();
        if result_live {
            then_live.insert(LiveSlot::Node(then_branch));
        }
        then_live = self.transfer(then_branch, then_live)?;
        let mut else_live = live;
        if let Some(else_branch) = else_branch {
            if result_live {
                else_live.insert(LiveSlot::Node(else_branch));
            }
            else_live = self.transfer(else_branch, else_live)?;
        }
        then_live.extend(else_live);
        self.operand(condition, then_live)
    }

    fn control_pattern(
        &mut self,
        subject: crate::CheckedPatternSubject,
        arms: &[crate::CheckedPatternArm],
        fallback: Option<crate::CheckedPatternFallback>,
        result_live: bool,
        live: LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        let mut joined = live.clone();
        for arm in arms {
            let mut arm_live = live.clone();
            if result_live {
                arm_live.insert(LiveSlot::Node(arm.body()));
            }
            arm_live = self.transfer(arm.body(), arm_live)?;
            for slot in arm.pattern().slots() {
                if let Some(binding) = slot.binding() {
                    Self::kill_root(&mut arm_live, PlaceRoot::Local(binding));
                }
            }
            joined.extend(arm_live);
        }
        if let Some(fallback) = fallback {
            let mut fallback_live = live;
            if result_live {
                fallback_live.insert(LiveSlot::Node(fallback.body()));
            }
            joined.extend(self.transfer(fallback.body(), fallback_live)?);
        }
        self.operand(subject.value(), joined)
    }

    fn loop_control(
        &mut self,
        loop_: LoopId,
        after: &LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        let definition = self
            .body
            .loops()
            .get(loop_)
            .cloned()
            .ok_or(crate::BodyCheckInternalError::UnknownLoop(loop_))?;
        let mut header = after.clone();
        loop {
            self.loops.push(LoopTargets {
                id: loop_,
                break_live: after.clone(),
                continue_live: header.clone(),
            });
            let mut body_live = self.transfer(definition.body(), header.clone())?;
            if let LoopKind::Range { binding, .. } | LoopKind::For { binding, .. } =
                definition.kind()
            {
                Self::kill_root(&mut body_live, PlaceRoot::Local(*binding));
            }
            let mut next = match definition.kind() {
                LoopKind::While { condition } => {
                    body_live.extend(after.iter().cloned());
                    self.operand(*condition, body_live)?
                }
                LoopKind::Infinite | LoopKind::Range { .. } | LoopKind::For { .. } => body_live,
            };
            let frame = self
                .loops
                .pop()
                .ok_or(crate::BodyCheckInternalError::LoopStack)?;
            if frame.id != loop_ {
                return Err(crate::BodyCheckInternalError::LoopStack);
            }
            if next == header {
                if let LoopKind::Range { start, end, .. } = definition.kind() {
                    next = self.operands([*start, *end], next)?;
                } else if let LoopKind::For { iteration, .. } = definition.kind() {
                    next = self.operand(iteration.source(), next)?;
                }
                return Ok(next);
            }
            header = next;
        }
    }

    fn loop_target(&self, loop_: LoopId) -> Result<&LoopTargets, crate::BodyCheckInternalError> {
        self.loops
            .iter()
            .rev()
            .find(|target| target.id == loop_)
            .ok_or(crate::BodyCheckInternalError::LoopStack)
    }

    fn cleanup_live(&self, node: BodyNodeId) -> Result<LiveSet, crate::BodyCheckInternalError> {
        let mut live = LiveSet::new();
        for schedule in self.body.cleanups().schedules(node).unwrap_or_default() {
            for action in schedule.actions() {
                let (ty, slot) = match action.target() {
                    CleanupTarget::Path(path) => (
                        path.ty(),
                        LiveSlot::Place(LivePlace::from_parts(path.root(), path.fields().into())),
                    ),
                    CleanupTarget::Place { place, ty } => {
                        let place = self
                            .body
                            .places()
                            .get(*place)
                            .ok_or(crate::BodyCheckInternalError::InvalidMovePlace(*place))?;
                        (*ty, LiveSlot::Place(LivePlace::from_checked(place)))
                    }
                    CleanupTarget::Value { node, ty }
                    | CleanupTarget::EnumResidual {
                        subject: node, ty, ..
                    } => (*ty, LiveSlot::Node(*node)),
                };
                if self.has_observing_drop(ty) {
                    live.insert(slot);
                }
            }
        }
        Ok(live)
    }

    fn has_observing_drop(&self, ty: nocter_model::TypeId) -> bool {
        matches!(
            self.types.get(ty),
            Some(nocter_model::TypeKind::Nominal { definition, .. })
                if self.drops.get(*definition).is_some()
        )
    }

    fn allocation(
        &mut self,
        selection: crate::AllocationSelection,
        live: LiveSet,
    ) -> Result<LiveSet, crate::BodyCheckInternalError> {
        match selection {
            crate::AllocationSelection::CurrentRegion => Ok(live),
            crate::AllocationSelection::Explicit(value) => self.operand(value, live),
        }
    }

    fn kill_place(
        &self,
        live: &mut LiveSet,
        place: nocter_model::PlaceId,
    ) -> Result<(), crate::BodyCheckInternalError> {
        let place = self
            .body
            .places()
            .get(place)
            .ok_or(crate::BodyCheckInternalError::InvalidMovePlace(place))?;
        let target = LivePlace::from_checked(place);
        live.retain(|slot| match slot {
            LiveSlot::Node(_) => true,
            LiveSlot::Place(candidate) => !candidate.is_at_or_below(&target),
        });
        Ok(())
    }

    fn kill_root(live: &mut LiveSet, root: PlaceRoot) {
        live.retain(|slot| !matches!(slot, LiveSlot::Place(place) if place.root() == root));
    }

    fn kill_scope(&self, live: &mut LiveSet, scope: nocter_model::BodyScopeId) {
        let roots = self
            .body
            .locals()
            .iter()
            .filter(|(_, local)| local.declaration().scope() == scope)
            .map(|(local, _)| PlaceRoot::Local(local))
            .collect::<BTreeSet<_>>();
        live.retain(
            |slot| !matches!(slot, LiveSlot::Place(place) if roots.contains(&place.root())),
        );
    }
}
