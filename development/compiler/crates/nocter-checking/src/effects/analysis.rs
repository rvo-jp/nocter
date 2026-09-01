use std::collections::{BTreeMap, HashSet};

use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_model::{
    AllocationGuarantee, ArenaBuilder, BodyNodeId, CallableGuarantees, CallableId, ClosureId,
    DropId, LoopId, PlaceId,
};
use nocter_toolchain_contract::StandardDeclarationRole;

use super::{AllocationEffect, EffectBodyInput, EffectTable, input_for_body};
use crate::{
    AggregateConstruction, AllocationSelection, ArgumentPackSegment, BodyCheckError,
    BodyCheckInternalError, BodyRule, BorrowConversionImplementation, CallTarget,
    CheckedArgumentPack, CheckedBody, CheckedControl, CheckedOperation, CheckedOutcome,
    CheckedReadonlyOperand, CheckedReceiver, CleanupAction, CleanupTarget, ClosureTable,
    ComparisonImplementation, InterpolationPart, IterationAcquisition, LoopKind, PlaceProjection,
    PlaceRoot, PrimitiveOperation, StaticDispatch, StaticSelection, TypedIteration,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Root {
    Callable(CallableId),
    Closure(ClosureId),
    Drop(DropId),
}

#[derive(Clone, Debug)]
enum EffectTarget {
    Callable(CallableId),
    Closure(ClosureId),
    Drop(DropId),
    Contract(CallableGuarantees),
}

#[derive(Clone, Debug, Default)]
struct RootFacts {
    direct_allocation: Option<BodyNodeId>,
    calls: Vec<(BodyNodeId, EffectTarget)>,
}

struct Summaries {
    callables: BTreeMap<CallableId, AllocationEffect>,
    closures: BTreeMap<ClosureId, AllocationEffect>,
    drops: BTreeMap<DropId, AllocationEffect>,
}

pub(super) fn analyze_program(
    environment: &crate::program_environment::ProgramEnvironment,
    closures: &ClosureTable,
    inputs: &[EffectBodyInput<'_, '_>],
) -> Result<EffectTable, BodyCheckError> {
    let graph = environment.graph();
    let facts = collect_facts(environment, closures, inputs)?;
    let mut summaries = initial_summaries(graph, closures);
    loop {
        let mut changed = false;
        for (root, root_facts) in &facts {
            if effect_cause(root_facts, &summaries).is_none() {
                continue;
            }
            let slot =
                summary_mut(&mut summaries, *root).ok_or(BodyCheckInternalError::EffectAnalysis)?;
            if !slot.may_allocate() {
                *slot = AllocationEffect::MayAllocate;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    validate_contracts(graph, closures, inputs, &facts, &summaries)?;
    freeze(summaries)
}

fn collect_facts(
    environment: &crate::program_environment::ProgramEnvironment,
    closures: &ClosureTable,
    inputs: &[EffectBodyInput<'_, '_>],
) -> Result<BTreeMap<Root, RootFacts>, BodyCheckError> {
    let graph = environment.graph();
    let allocation_request = environment
        .standard_semantics()
        .callable(StandardDeclarationRole::AllocationRequest);
    let mut facts = BTreeMap::new();
    for (body, declaration) in graph.declarations().bodies().iter() {
        let input =
            input_for_body(inputs, body).ok_or(BodyCheckInternalError::MissingBodySource(body))?;
        let root = match declaration.owner() {
            BodyOwner::Callable(callable) => Some(Root::Callable(callable)),
            BodyOwner::Drop(drop) => Some(Root::Drop(drop)),
            BodyOwner::Test(_) => None,
        };
        if let Some(root) = root {
            let mut root_facts =
                Collector::new(environment, input.body()).collect(input.body().root())?;
            if matches!(root, Root::Callable(callable) if Some(callable) == allocation_request) {
                root_facts.direct_allocation = Some(input.body().root());
            }
            facts.insert(root, root_facts);
        }
    }
    for (closure, definition) in closures.definitions().iter() {
        let input = input_for_body(inputs, definition.owner()).ok_or(
            BodyCheckInternalError::MissingBodySource(definition.owner()),
        )?;
        facts.insert(
            Root::Closure(closure),
            Collector::new(environment, input.body()).collect(definition.body())?,
        );
    }
    Ok(facts)
}

fn initial_summaries(graph: &DeclarationGraph, closures: &ClosureTable) -> Summaries {
    let callables = graph
        .declarations()
        .callables()
        .iter()
        .map(|(callable, declaration)| {
            (
                callable,
                if declaration.body().is_some() || guaranteed_noalloc(declaration.guarantees()) {
                    AllocationEffect::NoAllocation
                } else {
                    AllocationEffect::MayAllocate
                },
            )
        })
        .collect();
    let closure_summaries = closures
        .definitions()
        .iter()
        .map(|(closure, _)| (closure, AllocationEffect::NoAllocation))
        .collect();
    let drop_summaries = graph
        .declarations()
        .drops()
        .iter()
        .map(|(drop, _)| (drop, AllocationEffect::NoAllocation))
        .collect();
    Summaries {
        callables,
        closures: closure_summaries,
        drops: drop_summaries,
    }
}

fn effect_cause(facts: &RootFacts, summaries: &Summaries) -> Option<BodyNodeId> {
    facts.direct_allocation.or_else(|| {
        facts.calls.iter().find_map(|(node, target)| {
            target_effect(target, summaries)
                .may_allocate()
                .then_some(*node)
        })
    })
}

fn target_effect(target: &EffectTarget, summaries: &Summaries) -> AllocationEffect {
    match target {
        EffectTarget::Callable(callable) => summaries
            .callables
            .get(callable)
            .copied()
            .unwrap_or(AllocationEffect::MayAllocate),
        EffectTarget::Closure(closure) => summaries
            .closures
            .get(closure)
            .copied()
            .unwrap_or(AllocationEffect::MayAllocate),
        EffectTarget::Drop(drop) => summaries
            .drops
            .get(drop)
            .copied()
            .unwrap_or(AllocationEffect::MayAllocate),
        EffectTarget::Contract(guarantees) => {
            if guaranteed_noalloc(*guarantees) {
                AllocationEffect::NoAllocation
            } else {
                AllocationEffect::MayAllocate
            }
        }
    }
}

fn summary_mut(summaries: &mut Summaries, root: Root) -> Option<&mut AllocationEffect> {
    match root {
        Root::Callable(callable) => summaries.callables.get_mut(&callable),
        Root::Closure(closure) => summaries.closures.get_mut(&closure),
        Root::Drop(drop) => summaries.drops.get_mut(&drop),
    }
}

fn validate_contracts(
    graph: &DeclarationGraph,
    closures: &ClosureTable,
    inputs: &[EffectBodyInput<'_, '_>],
    facts: &BTreeMap<Root, RootFacts>,
    summaries: &Summaries,
) -> Result<(), BodyCheckError> {
    for (callable, declaration) in graph.declarations().callables().iter() {
        if !guaranteed_noalloc(declaration.guarantees())
            || !summaries
                .callables
                .get(&callable)
                .copied()
                .ok_or(BodyCheckInternalError::EffectAnalysis)?
                .may_allocate()
        {
            continue;
        }
        let body = declaration
            .body()
            .ok_or(BodyCheckInternalError::EffectAnalysis)?;
        return contract_error(
            inputs,
            body,
            effect_cause(
                facts
                    .get(&Root::Callable(callable))
                    .ok_or(BodyCheckInternalError::EffectAnalysis)?,
                summaries,
            )
            .ok_or(BodyCheckInternalError::EffectAnalysis)?,
        );
    }
    for (drop, declaration) in graph.declarations().drops().iter() {
        if !guaranteed_noalloc(declaration.guarantees())
            || !summaries
                .drops
                .get(&drop)
                .copied()
                .ok_or(BodyCheckInternalError::EffectAnalysis)?
                .may_allocate()
        {
            continue;
        }
        return contract_error(
            inputs,
            declaration.body(),
            effect_cause(
                facts
                    .get(&Root::Drop(drop))
                    .ok_or(BodyCheckInternalError::EffectAnalysis)?,
                summaries,
            )
            .ok_or(BodyCheckInternalError::EffectAnalysis)?,
        );
    }
    for (closure, definition) in closures.definitions().iter() {
        if !definition
            .callable_requirements()
            .iter()
            .any(|contract| guaranteed_noalloc(contract.guarantees()))
            || !summaries
                .closures
                .get(&closure)
                .copied()
                .ok_or(BodyCheckInternalError::EffectAnalysis)?
                .may_allocate()
        {
            continue;
        }
        return contract_error(
            inputs,
            definition.owner(),
            effect_cause(
                facts
                    .get(&Root::Closure(closure))
                    .ok_or(BodyCheckInternalError::EffectAnalysis)?,
                summaries,
            )
            .ok_or(BodyCheckInternalError::EffectAnalysis)?,
        );
    }
    Ok(())
}

fn contract_error(
    inputs: &[EffectBodyInput<'_, '_>],
    body: nocter_model::BodyId,
    node: BodyNodeId,
) -> Result<(), BodyCheckError> {
    let origin = input_for_body(inputs, body)
        .and_then(|input| input.origins().get(&node))
        .copied()
        .ok_or(BodyCheckInternalError::MissingNodeOrigin(node))?;
    let rule = BodyRule::NoAllocationContractViolation;
    Err(BodyCheckError::from_rule(rule, rule.diagnostic(origin)))
}

fn freeze(summaries: Summaries) -> Result<EffectTable, BodyCheckError> {
    let mut callables = ArenaBuilder::new();
    for (expected, effect) in summaries.callables {
        if callables.insert(effect) != expected {
            return Err(BodyCheckInternalError::EffectAnalysis.into());
        }
    }
    let mut closures = ArenaBuilder::new();
    for (expected, effect) in summaries.closures {
        if closures.insert(effect) != expected {
            return Err(BodyCheckInternalError::EffectAnalysis.into());
        }
    }
    let mut drops = ArenaBuilder::new();
    for (expected, effect) in summaries.drops {
        if drops.insert(effect) != expected {
            return Err(BodyCheckInternalError::EffectAnalysis.into());
        }
    }
    Ok(EffectTable::new(
        callables.finish(),
        closures.finish(),
        drops.finish(),
    ))
}

const fn guaranteed_noalloc(guarantees: CallableGuarantees) -> bool {
    matches!(guarantees.allocation(), AllocationGuarantee::NoAllocation)
}

struct Collector<'program> {
    graph: &'program DeclarationGraph,
    capability_evidence: &'program crate::body_check::CapabilityEvidenceTable,
    body: &'program CheckedBody,
    visited_nodes: HashSet<BodyNodeId>,
    visited_places: HashSet<PlaceId>,
    visited_loops: HashSet<LoopId>,
    facts: RootFacts,
}

impl<'program> Collector<'program> {
    fn new(
        environment: &'program crate::program_environment::ProgramEnvironment,
        body: &'program CheckedBody,
    ) -> Self {
        Self {
            graph: environment.graph(),
            capability_evidence: environment.capability_evidence(),
            body,
            visited_nodes: HashSet::new(),
            visited_places: HashSet::new(),
            visited_loops: HashSet::new(),
            facts: RootFacts::default(),
        }
    }

    fn collect(mut self, root: BodyNodeId) -> Result<RootFacts, BodyCheckError> {
        self.visit_node(root)?;
        Ok(self.facts)
    }

    fn visit_node(&mut self, node: BodyNodeId) -> Result<(), BodyCheckError> {
        if !self.visited_nodes.insert(node) {
            return Ok(());
        }
        let checked = self
            .body
            .nodes()
            .get(node)
            .cloned()
            .ok_or(BodyCheckInternalError::EffectAnalysis)?;
        if matches!(
            checked.operation(),
            CheckedOperation::Control(CheckedControl::Unreachable(_))
        ) {
            return Ok(());
        }
        self.visit_operation(node, checked.operation())?;
        if let Some(schedules) = self.body.cleanups().schedules(node).map(<[_]>::to_vec) {
            for schedule in schedules {
                for action in schedule.actions() {
                    self.visit_cleanup(node, action)?;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn visit_operation(
        &mut self,
        node: BodyNodeId,
        operation: &CheckedOperation,
    ) -> Result<(), BodyCheckError> {
        match operation {
            CheckedOperation::Complete
            | CheckedOperation::Constant(_)
            | CheckedOperation::ArgumentPackLength(_) => {}
            CheckedOperation::Place(place)
            | CheckedOperation::Copy(place)
            | CheckedOperation::Move(place)
            | CheckedOperation::Borrow { place, .. } => self.visit_place(*place)?,
            CheckedOperation::Call(call) => {
                match call.target() {
                    CallTarget::Static(selection) => self.record_selection(node, selection)?,
                    CallTarget::ClosureValue { value, closure, .. } => {
                        self.visit_node(*value)?;
                        self.facts
                            .calls
                            .push((node, EffectTarget::Closure(*closure)));
                    }
                    CallTarget::CallableValue {
                        value, dispatch, ..
                    } => {
                        self.visit_node(*value)?;
                        self.record_selection(node, dispatch)?;
                    }
                }
                if let Some(receiver) = call.receiver() {
                    self.visit_receiver(node, receiver)?;
                }
                for argument in call.arguments() {
                    self.visit_node(*argument)?;
                }
                if let Some(pack) = call.pack() {
                    self.visit_argument_pack(node, pack)?;
                }
            }
            CheckedOperation::BorrowConversion(conversion) => {
                self.visit_node(conversion.value())?;
                if let BorrowConversionImplementation::Selected(selection) =
                    conversion.implementation()
                {
                    self.record_selection(node, selection)?;
                }
            }
            CheckedOperation::CallableGuaranteeErasure(value) => self.visit_node(*value)?,
            CheckedOperation::Comparison(comparison) => {
                self.visit_readonly_operand(node, comparison.left())?;
                self.visit_readonly_operand(node, comparison.right())?;
                if let ComparisonImplementation::Selected(selection) = comparison.implementation() {
                    self.record_selection(node, selection)?;
                }
            }
            CheckedOperation::Primitive(primitive) => match primitive {
                PrimitiveOperation::Unary { operand, .. }
                | PrimitiveOperation::IntegerConversion { operand, .. } => {
                    self.visit_node(*operand)?;
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
            CheckedOperation::OpaqueWitness(witness) => self.visit_node(witness.value())?,
            CheckedOperation::Closure(closure) => {
                for capture in closure.captures() {
                    self.visit_node(capture.initializer())?;
                }
            }
            CheckedOperation::IteratorAcquisition(acquisition) => {
                self.visit_receiver(node, acquisition.source())?;
                if let IterationAcquisition::Expansion(selection) = acquisition.acquisition() {
                    self.record_selection(node, selection)?;
                }
            }
            CheckedOperation::PackLiteral(literal) => {
                self.record_direct_allocation(node);
                self.visit_argument_pack(node, literal.pack())?;
                self.visit_allocation(literal.allocation())?;
            }
            CheckedOperation::StringLiteral { allocation, .. } => {
                self.record_direct_allocation(node);
                self.visit_allocation(*allocation)?;
            }
            CheckedOperation::Interpolation(interpolation) => {
                self.record_direct_allocation(node);
                for part in interpolation.parts() {
                    match part {
                        InterpolationPart::Text(_) => {}
                        InterpolationPart::Formatted { operand, formatter } => {
                            self.visit_readonly_operand(node, operand)?;
                            self.record_selection(node, formatter)?;
                        }
                        InterpolationPart::Diverging(value) => self.visit_node(*value)?,
                    }
                }
                self.visit_allocation(interpolation.allocation())?;
            }
            CheckedOperation::Control(control) => self.visit_control(node, control)?,
        }
        Ok(())
    }

    fn visit_receiver(
        &mut self,
        node: BodyNodeId,
        receiver: &CheckedReceiver,
    ) -> Result<(), BodyCheckError> {
        self.visit_node(receiver.value())?;
        if let Some(coercion) = receiver.coercion() {
            self.record_selection(node, coercion.selection())?;
        }
        Ok(())
    }

    fn visit_readonly_operand(
        &mut self,
        node: BodyNodeId,
        operand: &CheckedReadonlyOperand,
    ) -> Result<(), BodyCheckError> {
        self.visit_node(operand.value())?;
        if let Some(coercion) = operand.coercion() {
            self.record_selection(node, coercion)?;
        }
        Ok(())
    }

    fn visit_iteration(
        &mut self,
        node: BodyNodeId,
        iteration: &TypedIteration,
    ) -> Result<(), BodyCheckError> {
        self.visit_node(iteration.iterator())?;
        self.record_selection(node, iteration.next())
    }

    fn visit_argument_pack(
        &mut self,
        node: BodyNodeId,
        pack: &CheckedArgumentPack,
    ) -> Result<(), BodyCheckError> {
        for segment in pack.segments() {
            match segment {
                ArgumentPackSegment::Value(value) => self.visit_node(*value)?,
                ArgumentPackSegment::KeyedValue { key, value } => {
                    self.visit_node(*key)?;
                    self.visit_node(*value)?;
                }
                ArgumentPackSegment::Spread {
                    iteration,
                    exact_size,
                    ..
                } => {
                    self.visit_iteration(node, iteration)?;
                    self.record_selection(node, exact_size)?;
                }
            }
        }
        Ok(())
    }

    fn visit_allocation(&mut self, allocation: AllocationSelection) -> Result<(), BodyCheckError> {
        if let AllocationSelection::Explicit(value) = allocation {
            self.visit_node(value)?;
        }
        Ok(())
    }

    fn visit_outcome(&mut self, outcome: &CheckedOutcome) -> Result<(), BodyCheckError> {
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
                operand, fallback, ..
            } => {
                self.visit_node(*operand)?;
                self.visit_node(*fallback)?;
            }
        }
        Ok(())
    }

    fn visit_control(
        &mut self,
        node: BodyNodeId,
        control: &CheckedControl,
    ) -> Result<(), BodyCheckError> {
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
            CheckedControl::Discard(value) => self.visit_node(*value)?,
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
                        self.facts
                            .calls
                            .push((node, EffectTarget::Drop(drop.declaration())));
                    }
                    self.visit_node(arm.body())?;
                }
                if let Some(fallback) = fallback.filter(|fallback| fallback.reachable()) {
                    self.visit_node(fallback.body())?;
                }
            }
            CheckedControl::Loop(loop_) => self.visit_loop(node, *loop_)?,
            CheckedControl::Region {
                allocator, body, ..
            } => {
                self.visit_node(*allocator)?;
                self.visit_node(*body)?;
            }
        }
        Ok(())
    }

    fn visit_loop(&mut self, node: BodyNodeId, loop_: LoopId) -> Result<(), BodyCheckError> {
        if !self.visited_loops.insert(loop_) {
            return Ok(());
        }
        let loop_ = self
            .body
            .loops()
            .get(loop_)
            .ok_or(BodyCheckInternalError::EffectAnalysis)?;
        match loop_.kind() {
            LoopKind::Infinite
            | LoopKind::ArgumentPack { .. }
            | LoopKind::KeyedArgumentPack { .. } => {}
            LoopKind::While { condition } => self.visit_node(*condition)?,
            LoopKind::For { iteration, .. } => self.visit_iteration(node, iteration)?,
            LoopKind::Range { start, end, .. } => {
                self.visit_node(*start)?;
                self.visit_node(*end)?;
            }
        }
        self.visit_node(loop_.body())
    }

    fn visit_place(&mut self, place: PlaceId) -> Result<(), BodyCheckError> {
        if !self.visited_places.insert(place) {
            return Ok(());
        }
        let place = self
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::EffectAnalysis)?;
        if let PlaceRoot::Value(value) = place.root() {
            self.visit_node(value)?;
        }
        for projection in place.projections() {
            match projection {
                PlaceProjection::Field { .. } | PlaceProjection::BorrowDeref { .. } => {}
                PlaceProjection::BuiltinIndex { index, .. } => self.visit_node(*index)?,
                PlaceProjection::CoercedBuiltinIndex {
                    index,
                    receiver_coercion,
                    ..
                } => {
                    self.visit_node(*index)?;
                    self.record_selection(*index, receiver_coercion)?;
                }
                PlaceProjection::SelectedIndex {
                    index,
                    operation,
                    receiver_coercion,
                    ..
                } => {
                    self.visit_node(*index)?;
                    if let Some(coercion) = receiver_coercion {
                        self.record_selection(*index, coercion)?;
                    }
                    self.record_selection(*index, operation)?;
                }
            }
        }
        Ok(())
    }

    fn visit_cleanup(
        &mut self,
        site: BodyNodeId,
        action: &CleanupAction,
    ) -> Result<(), BodyCheckError> {
        match action.target() {
            CleanupTarget::Path(_) => {}
            CleanupTarget::Place { place, .. } => {
                self.visit_place(*place)?;
            }
            CleanupTarget::Value { node, .. } => {
                self.visit_node(*node)?;
            }
            CleanupTarget::EnumResidual { subject, .. } => {
                self.visit_node(*subject)?;
            }
            CleanupTarget::Region { parent, .. } => self.visit_node(*parent)?,
        }
        for drop in action.effect().drops() {
            self.facts.calls.push((site, EffectTarget::Drop(*drop)));
        }
        if action.effect().has_unknown_destruction() {
            self.facts
                .calls
                .push((site, EffectTarget::Contract(CallableGuarantees::default())));
        }
        Ok(())
    }

    fn record_direct_allocation(&mut self, node: BodyNodeId) {
        self.facts.direct_allocation.get_or_insert(node);
    }

    fn record_selection(
        &mut self,
        node: BodyNodeId,
        selection: &StaticSelection,
    ) -> Result<(), BodyCheckError> {
        let target = match selection.dispatch() {
            StaticDispatch::Direct(callable)
            | StaticDispatch::InterfaceDefault {
                method: callable, ..
            } => EffectTarget::Callable(callable),
            StaticDispatch::InterfaceMethod { method, .. }
            | StaticDispatch::InterfaceSelfMethod { method, .. }
            | StaticDispatch::OpaqueMethod { method, .. } => {
                let guarantees = self
                    .graph
                    .declarations()
                    .callables()
                    .get(method)
                    .map(nocter_declarations::CallableDeclaration::guarantees)
                    .ok_or(BodyCheckInternalError::EffectAnalysis)?;
                EffectTarget::Contract(guarantees)
            }
            StaticDispatch::StructuralRequirement { evidence } => EffectTarget::Contract(
                match self
                    .capability_evidence
                    .get(evidence)
                    .map(crate::body_check::CapabilityEvidence::predicate)
                {
                    Some(crate::CheckedPredicate::Callable { contract, .. }) => {
                        contract.guarantees()
                    }
                    Some(_) => CallableGuarantees::default(),
                    None => return Err(BodyCheckInternalError::EffectAnalysis.into()),
                },
            ),
        };
        self.facts.calls.push((node, target));
        Ok(())
    }
}
