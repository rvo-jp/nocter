use std::collections::{BTreeMap, HashSet};

use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_model::{
    AllocationGuarantee, ArenaBuilder, BodyNodeId, CallableGuarantees, CallableId, ClosureId,
    DropId, LoopId, NonblockingGuarantee, PlaceId, TypeStore,
};
use nocter_toolchain_contract::StandardDeclarationRole;

use super::{AllocationFact, ExecutionFactTable, ExecutionFacts};
use crate::body_relations::BodyRelationCatalog;
use crate::{
    AggregateConstruction, AllocationSelection, ArgumentPackSegment, BodyCheckInternalError,
    BodyRelationError, BodyRule, BorrowConversionImplementation, CallTarget, CheckedArgumentPack,
    CheckedBody, CheckedControl, CheckedOperation, CheckedOutcome, CheckedReadonlyOperand,
    CheckedReceiver, CleanupAction, CleanupTarget, ClosureTable, InterpolationPart,
    IterationAcquisition, LoopKind, PlaceProjection, PlaceRoot, PrimitiveOperation, StaticDispatch,
    StaticSelection,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Root {
    Callable(CallableId),
    Closure(ClosureId),
    Drop(DropId),
}

#[derive(Clone, Debug)]
enum ExecutionTarget {
    Callable(CallableId),
    Closure(ClosureId),
    Drop(DropId),
    /// An implementation unavailable by design and bounded only by its authored contract.
    ExternalContract(CallableGuarantees),
}

#[derive(Clone, Debug, Default)]
struct RootRelations {
    direct: Vec<(BodyNodeId, ExecutionFacts)>,
    executions: Vec<(BodyNodeId, ExecutionTarget)>,
}

struct Summaries {
    callables: BTreeMap<CallableId, ExecutionFacts>,
    closures: BTreeMap<ClosureId, ExecutionFacts>,
    drops: BTreeMap<DropId, ExecutionFacts>,
}

pub(super) fn analyze_program(
    environment: &crate::program_environment::ProgramEnvironment,
    types: &TypeStore,
    closures: &ClosureTable,
    inputs: &BodyRelationCatalog<'_>,
) -> Result<ExecutionFactTable, BodyRelationError> {
    let graph = environment.graph();
    let facts = collect_facts(environment, types, closures, inputs)?;
    let mut summaries = initial_summaries(graph, closures);
    loop {
        let mut changed = false;
        for (root, root_facts) in &facts {
            let reachable = inferred_facts(root_facts, &summaries)?;
            let slot = summary_mut(&mut summaries, *root)
                .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
            changed |= slot.include(reachable);
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
    types: &TypeStore,
    closures: &ClosureTable,
    inputs: &BodyRelationCatalog<'_>,
) -> Result<BTreeMap<Root, RootRelations>, BodyRelationError> {
    let graph = environment.graph();
    let allocation_request = environment
        .standard_semantics()
        .callable(StandardDeclarationRole::AllocationRequest);
    let mut facts = BTreeMap::new();
    for (body, declaration) in graph.declarations().bodies().iter() {
        let input = inputs.get(body)?;
        let root = match declaration.owner() {
            BodyOwner::Callable(callable) => Some(Root::Callable(callable)),
            BodyOwner::Drop(drop) => Some(Root::Drop(drop)),
            BodyOwner::Constant(_) | BodyOwner::Static(_) | BodyOwner::Test(_) => None,
        };
        if let Some(root) = root {
            let mut root_facts =
                Collector::new(environment, types, input.body()).collect(input.body().root())?;
            if matches!(root, Root::Callable(callable) if Some(callable) == allocation_request) {
                root_facts
                    .direct
                    .push((input.body().root(), ExecutionFacts::allocation_request()));
            }
            facts.insert(root, root_facts);
        }
    }
    for (closure, definition) in closures.definitions().iter() {
        let input = inputs.get(definition.owner())?;
        facts.insert(
            Root::Closure(closure),
            Collector::new(environment, types, input.body()).collect(definition.body())?,
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
                initial_callable_facts(declaration.body().is_none(), declaration.guarantees()),
            )
        })
        .collect();
    let closure_summaries = closures
        .definitions()
        .iter()
        .map(|(closure, _)| (closure, ExecutionFacts::default()))
        .collect();
    let drop_summaries = graph
        .declarations()
        .drops()
        .iter()
        .map(|(drop, _)| (drop, ExecutionFacts::default()))
        .collect();
    Summaries {
        callables,
        closures: closure_summaries,
        drops: drop_summaries,
    }
}

fn initial_callable_facts(bodyless: bool, guarantees: CallableGuarantees) -> ExecutionFacts {
    let admitted = ExecutionFacts::admitted_by(guarantees);
    ExecutionFacts::new(
        if bodyless {
            admitted.allocation()
        } else {
            AllocationFact::NoAllocation
        },
        admitted.synchronous_wait(),
    )
}

fn inferred_facts(
    relations: &RootRelations,
    summaries: &Summaries,
) -> Result<ExecutionFacts, BodyRelationError> {
    let mut facts = ExecutionFacts::default();
    for (_, direct) in &relations.direct {
        facts.include(*direct);
    }
    for (_, target) in &relations.executions {
        facts.include(target_facts(target, summaries)?);
    }
    Ok(facts)
}

fn allocation_cause(
    relations: &RootRelations,
    summaries: &Summaries,
) -> Result<Option<BodyNodeId>, BodyRelationError> {
    execution_cause(relations, summaries, |facts| {
        facts.allocation().may_allocate()
    })
}

fn blocking_cause(
    relations: &RootRelations,
    summaries: &Summaries,
) -> Result<Option<BodyNodeId>, BodyRelationError> {
    execution_cause(relations, summaries, |facts| {
        facts.synchronous_wait().may_block()
    })
}

fn execution_cause(
    relations: &RootRelations,
    summaries: &Summaries,
    matches: impl Fn(ExecutionFacts) -> bool,
) -> Result<Option<BodyNodeId>, BodyRelationError> {
    for (node, direct) in &relations.direct {
        if matches(*direct) {
            return Ok(Some(*node));
        }
    }
    for (node, target) in &relations.executions {
        if matches(target_facts(target, summaries)?) {
            return Ok(Some(*node));
        }
    }
    Ok(None)
}

fn target_facts(
    target: &ExecutionTarget,
    summaries: &Summaries,
) -> Result<ExecutionFacts, BodyRelationError> {
    match target {
        ExecutionTarget::Callable(callable) => summaries
            .callables
            .get(callable)
            .copied()
            .ok_or_else(|| BodyCheckInternalError::ExecutionAnalysis.into()),
        ExecutionTarget::Closure(closure) => summaries
            .closures
            .get(closure)
            .copied()
            .ok_or_else(|| BodyCheckInternalError::ExecutionAnalysis.into()),
        ExecutionTarget::Drop(drop) => summaries
            .drops
            .get(drop)
            .copied()
            .ok_or_else(|| BodyCheckInternalError::ExecutionAnalysis.into()),
        ExecutionTarget::ExternalContract(guarantees) => {
            Ok(ExecutionFacts::admitted_by(*guarantees))
        }
    }
}

fn summary_mut(summaries: &mut Summaries, root: Root) -> Option<&mut ExecutionFacts> {
    match root {
        Root::Callable(callable) => summaries.callables.get_mut(&callable),
        Root::Closure(closure) => summaries.closures.get_mut(&closure),
        Root::Drop(drop) => summaries.drops.get_mut(&drop),
    }
}

fn validate_contracts(
    graph: &DeclarationGraph,
    closures: &ClosureTable,
    inputs: &BodyRelationCatalog<'_>,
    facts: &BTreeMap<Root, RootRelations>,
    summaries: &Summaries,
) -> Result<(), BodyRelationError> {
    for (callable, declaration) in graph.declarations().callables().iter() {
        let execution = summaries
            .callables
            .get(&callable)
            .copied()
            .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
        let Some(body) = declaration.body() else {
            continue;
        };
        let root_facts = facts
            .get(&Root::Callable(callable))
            .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
        if guaranteed_noalloc(declaration.guarantees()) && execution.allocation().may_allocate() {
            return contract_error(
                inputs,
                body,
                allocation_cause(root_facts, summaries)?
                    .ok_or(BodyCheckInternalError::ExecutionAnalysis)?,
                BodyRule::NoAllocationContractViolation,
            );
        }
        if guaranteed_nonblocking(declaration.guarantees())
            && execution.synchronous_wait().may_block()
        {
            return contract_error(
                inputs,
                body,
                blocking_cause(root_facts, summaries)?
                    .ok_or(BodyCheckInternalError::ExecutionAnalysis)?,
                BodyRule::BlockingContractViolation,
            );
        }
    }
    for (drop, declaration) in graph.declarations().drops().iter() {
        let execution = summaries
            .drops
            .get(&drop)
            .copied()
            .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
        let root_facts = facts
            .get(&Root::Drop(drop))
            .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
        if guaranteed_noalloc(declaration.guarantees()) && execution.allocation().may_allocate() {
            return contract_error(
                inputs,
                declaration.body(),
                allocation_cause(root_facts, summaries)?
                    .ok_or(BodyCheckInternalError::ExecutionAnalysis)?,
                BodyRule::NoAllocationContractViolation,
            );
        }
        if execution.synchronous_wait().may_block() {
            return contract_error(
                inputs,
                declaration.body(),
                blocking_cause(root_facts, summaries)?
                    .ok_or(BodyCheckInternalError::ExecutionAnalysis)?,
                BodyRule::BlockingContractViolation,
            );
        }
    }
    for (closure, definition) in closures.definitions().iter() {
        let execution = summaries
            .closures
            .get(&closure)
            .copied()
            .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
        let root_facts = facts
            .get(&Root::Closure(closure))
            .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
        if definition
            .callable_requirements()
            .iter()
            .any(|contract| guaranteed_noalloc(contract.guarantees()))
            && execution.allocation().may_allocate()
        {
            return contract_error(
                inputs,
                definition.owner(),
                allocation_cause(root_facts, summaries)?
                    .ok_or(BodyCheckInternalError::ExecutionAnalysis)?,
                BodyRule::NoAllocationContractViolation,
            );
        }
        if definition
            .callable_requirements()
            .iter()
            .any(|contract| guaranteed_nonblocking(contract.guarantees()))
            && execution.synchronous_wait().may_block()
        {
            return contract_error(
                inputs,
                definition.owner(),
                blocking_cause(root_facts, summaries)?
                    .ok_or(BodyCheckInternalError::ExecutionAnalysis)?,
                BodyRule::BlockingContractViolation,
            );
        }
    }
    Ok(())
}

fn contract_error(
    inputs: &BodyRelationCatalog<'_>,
    body: nocter_model::BodyId,
    node: BodyNodeId,
    rule: BodyRule,
) -> Result<(), BodyRelationError> {
    let input = inputs.get(body)?;
    Err(input.reject(rule, node))
}

fn freeze(summaries: Summaries) -> Result<ExecutionFactTable, BodyRelationError> {
    let mut callables = ArenaBuilder::new();
    for (expected, facts) in summaries.callables {
        if callables.insert(facts) != expected {
            return Err(BodyCheckInternalError::ExecutionAnalysis.into());
        }
    }
    let mut closures = ArenaBuilder::new();
    for (expected, facts) in summaries.closures {
        if closures.insert(facts) != expected {
            return Err(BodyCheckInternalError::ExecutionAnalysis.into());
        }
    }
    let mut drops = ArenaBuilder::new();
    for (expected, facts) in summaries.drops {
        if drops.insert(facts) != expected {
            return Err(BodyCheckInternalError::ExecutionAnalysis.into());
        }
    }
    Ok(ExecutionFactTable::new(
        callables.finish(),
        closures.finish(),
        drops.finish(),
    ))
}

const fn guaranteed_noalloc(guarantees: CallableGuarantees) -> bool {
    matches!(guarantees.allocation(), AllocationGuarantee::NoAllocation)
}

const fn guaranteed_nonblocking(guarantees: CallableGuarantees) -> bool {
    matches!(guarantees.nonblocking(), NonblockingGuarantee::Nonblocking)
}

struct Collector<'program> {
    graph: &'program DeclarationGraph,
    types: &'program TypeStore,
    capability_evidence: &'program crate::body_check::CapabilityEvidenceTable,
    body: &'program CheckedBody,
    visited_nodes: HashSet<BodyNodeId>,
    visited_places: HashSet<PlaceId>,
    visited_loops: HashSet<LoopId>,
    facts: RootRelations,
}

impl<'program> Collector<'program> {
    fn new(
        environment: &'program crate::program_environment::ProgramEnvironment,
        types: &'program TypeStore,
        body: &'program CheckedBody,
    ) -> Self {
        Self {
            graph: environment.graph(),
            types,
            capability_evidence: environment.capability_evidence(),
            body,
            visited_nodes: HashSet::new(),
            visited_places: HashSet::new(),
            visited_loops: HashSet::new(),
            facts: RootRelations::default(),
        }
    }

    fn collect(mut self, root: BodyNodeId) -> Result<RootRelations, BodyRelationError> {
        self.visit_node(root)?;
        Ok(self.facts)
    }

    fn visit_node(&mut self, node: BodyNodeId) -> Result<(), BodyRelationError> {
        if !self.visited_nodes.insert(node) {
            return Ok(());
        }
        let checked = self
            .body
            .nodes()
            .get(node)
            .cloned()
            .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
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
    ) -> Result<(), BodyRelationError> {
        match operation {
            CheckedOperation::Complete
            | CheckedOperation::Literal(_)
            | CheckedOperation::DeclaredConstant(_)
            | CheckedOperation::ArgumentPackLength(_) => {}
            CheckedOperation::Place(place)
            | CheckedOperation::Copy(place)
            | CheckedOperation::Move(place)
            | CheckedOperation::Borrow { place, .. } => self.visit_place(*place)?,
            CheckedOperation::Call(call) => {
                let deferred = call.execution().is_deferred();
                match call.target() {
                    CallTarget::Static(selection) => {
                        if !deferred {
                            self.record_selection(node, selection)?;
                        }
                    }
                    CallTarget::ClosureValue { value, closure, .. } => {
                        self.visit_node(*value)?;
                        if !deferred {
                            self.facts
                                .executions
                                .push((node, ExecutionTarget::Closure(*closure)));
                        }
                    }
                    CallTarget::CallableValue {
                        value, dispatch, ..
                    } => {
                        self.visit_node(*value)?;
                        if !deferred {
                            self.record_selection(node, dispatch)?;
                        }
                    }
                    CallTarget::ErasedCallableValue { value, .. } => {
                        self.visit_node(*value)?;
                        if !deferred {
                            let checked = self
                                .body
                                .nodes()
                                .get(*value)
                                .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
                            let Some(nocter_model::TypeKind::Callable(callable)) =
                                self.types.get(checked.ty())
                            else {
                                return Err(BodyCheckInternalError::ExecutionAnalysis.into());
                            };
                            self.facts.executions.push((
                                node,
                                ExecutionTarget::ExternalContract(callable.contract().guarantees()),
                            ));
                        }
                    }
                }
                if deferred {
                    self.record_direct_allocation(node);
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
            CheckedOperation::Await(await_) => self.visit_node(await_.computation())?,
            CheckedOperation::CallableGuaranteeErasure(value) => self.visit_node(*value)?,
            CheckedOperation::CallableErasure(erasure) => {
                self.visit_node(erasure.value())?;
                self.record_direct_allocation(node);
            }
            CheckedOperation::Comparison(comparison) => {
                self.visit_readonly_operand(node, comparison.left())?;
                self.visit_readonly_operand(node, comparison.right())?;
                for step in comparison.plan().steps() {
                    for selection in step.selections() {
                        self.record_selection(node, selection)?;
                    }
                }
            }
            CheckedOperation::Primitive(primitive) => match primitive {
                PrimitiveOperation::Unary { operand, .. }
                | PrimitiveOperation::NumericConversion { operand, .. } => {
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
                | AggregateConstruction::FixedArray(payload)
                | AggregateConstruction::Tuple(payload) => {
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
    ) -> Result<(), BodyRelationError> {
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
    ) -> Result<(), BodyRelationError> {
        self.visit_node(operand.value())?;
        if let Some(coercion) = operand.coercion() {
            self.record_selection(node, coercion)?;
        }
        Ok(())
    }

    fn visit_iteration(
        &mut self,
        node: BodyNodeId,
        iteration: &crate::TypedIterationStep,
    ) -> Result<(), BodyRelationError> {
        self.visit_node(iteration.iterator())?;
        self.record_selection(node, iteration.next())
    }

    fn visit_argument_pack(
        &mut self,
        node: BodyNodeId,
        pack: &CheckedArgumentPack,
    ) -> Result<(), BodyRelationError> {
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
                    self.visit_iteration(node, iteration.step())?;
                    self.record_selection(node, exact_size)?;
                }
            }
        }
        Ok(())
    }

    fn visit_allocation(
        &mut self,
        allocation: AllocationSelection,
    ) -> Result<(), BodyRelationError> {
        if let AllocationSelection::Explicit(value) = allocation {
            self.visit_node(value)?;
        }
        Ok(())
    }

    fn visit_outcome(&mut self, outcome: &CheckedOutcome) -> Result<(), BodyRelationError> {
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
    ) -> Result<(), BodyRelationError> {
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
                            .executions
                            .push((node, ExecutionTarget::Drop(drop.declaration())));
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

    fn visit_loop(&mut self, node: BodyNodeId, loop_: LoopId) -> Result<(), BodyRelationError> {
        if !self.visited_loops.insert(loop_) {
            return Ok(());
        }
        let loop_ = self
            .body
            .loops()
            .get(loop_)
            .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
        match loop_.kind() {
            LoopKind::Infinite
            | LoopKind::ArgumentPack { .. }
            | LoopKind::KeyedArgumentPack { .. } => {}
            LoopKind::While { condition } => self.visit_node(*condition)?,
            LoopKind::For { iteration, .. } => self.visit_iteration(node, iteration.step())?,
            LoopKind::ForAwait { iteration, .. } => self.visit_iteration(node, iteration.step())?,
            LoopKind::Range { start, end, .. } => {
                self.visit_node(*start)?;
                self.visit_node(*end)?;
            }
        }
        self.visit_node(loop_.body())
    }

    fn visit_place(&mut self, place: PlaceId) -> Result<(), BodyRelationError> {
        if !self.visited_places.insert(place) {
            return Ok(());
        }
        let place = self
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
        if let PlaceRoot::Value(value) = place.root() {
            self.visit_node(value)?;
        }
        for projection in place.projections() {
            match projection {
                PlaceProjection::Field { .. }
                | PlaceProjection::TupleElement { .. }
                | PlaceProjection::BorrowDeref { .. } => {}
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
    ) -> Result<(), BodyRelationError> {
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
        for drop in action.dependencies().drops() {
            self.facts
                .executions
                .push((site, ExecutionTarget::Drop(*drop)));
        }
        if action.dependencies().has_unknown_destruction() {
            self.facts.executions.push((
                site,
                ExecutionTarget::ExternalContract(CallableGuarantees::default()),
            ));
        }
        Ok(())
    }

    fn record_direct_allocation(&mut self, node: BodyNodeId) {
        self.facts
            .direct
            .push((node, ExecutionFacts::allocation_request()));
    }

    fn record_selection(
        &mut self,
        node: BodyNodeId,
        selection: &StaticSelection,
    ) -> Result<(), BodyRelationError> {
        let target = match selection.dispatch() {
            StaticDispatch::Direct(callable)
            | StaticDispatch::InterfaceDefault {
                method: callable, ..
            } => ExecutionTarget::Callable(callable),
            StaticDispatch::InterfaceMethod { method, .. }
            | StaticDispatch::InterfaceSelfMethod { method, .. }
            | StaticDispatch::OpaqueMethod { method, .. } => {
                let guarantees = self
                    .graph
                    .declarations()
                    .callables()
                    .get(method)
                    .map(nocter_declarations::CallableDeclaration::guarantees)
                    .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
                ExecutionTarget::ExternalContract(guarantees)
            }
            StaticDispatch::StructuralRequirement { evidence } => {
                let predicate = self
                    .capability_evidence
                    .get(evidence)
                    .map(crate::body_check::CapabilityEvidence::predicate)
                    .ok_or(BodyCheckInternalError::ExecutionAnalysis)?;
                ExecutionTarget::ExternalContract(structural_execution_guarantees(predicate)?)
            }
        };
        self.facts.executions.push((node, target));
        Ok(())
    }
}

fn structural_execution_guarantees(
    predicate: &crate::CheckedPredicate,
) -> Result<CallableGuarantees, BodyRelationError> {
    match predicate {
        crate::CheckedPredicate::Callable { contract, .. } => Ok(contract.guarantees()),
        crate::CheckedPredicate::Equality(_)
        | crate::CheckedPredicate::Ordering(_)
        | crate::CheckedPredicate::Index { .. }
        | crate::CheckedPredicate::Coercion { .. }
        | crate::CheckedPredicate::Expansion { .. } => Ok(CallableGuarantees::default()),
        crate::CheckedPredicate::Interface { .. }
        | crate::CheckedPredicate::Copy(_)
        | crate::CheckedPredicate::BinderRefinement { .. } => {
            Err(BodyCheckInternalError::ExecutionAnalysis.into())
        }
    }
}
