use std::collections::{BTreeMap, BTreeSet, HashMap};

use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_model::{
    ArenaBuilder, BodyId, BodyNodeId, BorrowCapability, ClosureId, LoopId, ParameterOrigin,
    TypeKind, TypeStore,
};
mod access;
mod calls;
mod cleanup;
mod control;
mod values;

use super::liveness::Liveness;
use super::state::LoanState;
use super::value::LoanValue;
use crate::{
    BodyCheckInternalError, BodyRelationError, CheckedBodyLoans, CheckedLoan, CheckedOperation,
    ClosureDefinition, ClosureTable, DropTable, LoanId, LoanPlace, LoanRoot, LoanTable, PlaceRoot,
    ProvenanceTable, SuspensionStorage,
    body_relations::{BodyRelationCatalog, BodyRelationInput},
};

#[derive(Clone, Copy)]
struct ProgramFacts<'program> {
    graph: &'program DeclarationGraph,
    types: &'program TypeStore,
    capability_evidence: &'program crate::body_check::CapabilityEvidenceTable,
    drops: &'program DropTable,
    provenance: &'program ProvenanceTable,
}

pub(super) fn analyze_program(
    graph: &DeclarationGraph,
    types: &TypeStore,
    capability_evidence: &crate::body_check::CapabilityEvidenceTable,
    drops: &DropTable,
    provenance: &ProvenanceTable,
    closures: &ClosureTable,
    inputs: &BodyRelationCatalog<'_>,
) -> Result<LoanTable, BodyRelationError> {
    let facts = ProgramFacts {
        graph,
        types,
        capability_evidence,
        drops,
        provenance,
    };
    let mut bodies = ArenaBuilder::<BodyId, CheckedBodyLoans>::new();
    for (body, _) in graph.declarations().bodies().iter() {
        let input = inputs.get(body)?;
        let liveness = super::liveness::analyze(types, drops, input.body(), input.body().root())?;
        let mut checked = Analyzer::new(facts, input, &liveness, None).analyze()?;
        for (closure, definition) in closures
            .definitions()
            .iter()
            .filter(|(_, definition)| definition.owner() == body)
        {
            let liveness = super::liveness::analyze(types, drops, input.body(), definition.body())?;
            let closure_checked =
                Analyzer::new(facts, input, &liveness, Some((closure, definition))).analyze()?;
            checked.merge(closure_checked)?;
        }
        let mut live_before = ArenaBuilder::new();
        for (node, _) in input.body().nodes().iter() {
            let loans = checked
                .live_before
                .remove(&node)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let actual = live_before.insert(loans);
            if actual != node {
                return Err(BodyCheckInternalError::LoanAnalysis.into());
            }
        }
        if !checked.live_before.is_empty() {
            return Err(BodyCheckInternalError::LoanAnalysis.into());
        }
        let suspension_storage = checked
            .suspension_storage
            .into_iter()
            .map(|(node, storage)| {
                (
                    node,
                    storage.into_iter().collect::<Vec<_>>().into_boxed_slice(),
                )
            })
            .collect();
        let checked =
            CheckedBodyLoans::new(checked.loans, live_before.finish(), suspension_storage);
        let actual = bodies.insert(checked);
        if actual != body {
            return Err(BodyCheckInternalError::LoanAnalysis.into());
        }
    }
    Ok(LoanTable::new(bodies.finish()))
}

struct RootLoanAnalysis {
    loans: BTreeMap<LoanId, CheckedLoan>,
    live_before: HashMap<BodyNodeId, BTreeSet<LoanId>>,
    suspension_storage: BTreeMap<BodyNodeId, BTreeSet<SuspensionStorage>>,
}

impl RootLoanAnalysis {
    fn merge(&mut self, another: Self) -> Result<(), BodyRelationError> {
        for (loan, definition) in another.loans {
            if self.loans.insert(loan, definition).is_some() {
                return Err(BodyCheckInternalError::LoanAnalysis.into());
            }
        }
        for (node, loans) in another.live_before {
            self.live_before.entry(node).or_default().extend(loans);
        }
        for (node, storage) in another.suspension_storage {
            self.suspension_storage
                .entry(node)
                .or_default()
                .extend(storage);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum AccessKind {
    Read,
    Write,
    Borrow(BorrowCapability),
}

struct LoopFlow {
    id: LoopId,
    scope_depth: usize,
    breaks: Vec<LoanState>,
    continues: Vec<LoanState>,
}

struct Analyzer<'program> {
    graph: &'program DeclarationGraph,
    types: &'program TypeStore,
    capability_evidence: &'program crate::body_check::CapabilityEvidenceTable,
    drops: &'program DropTable,
    provenance: &'program ProvenanceTable,
    input: &'program BodyRelationInput<'program>,
    liveness: &'program Liveness,
    loans: BTreeMap<LoanId, CheckedLoan>,
    live_before: HashMap<BodyNodeId, BTreeSet<LoanId>>,
    suspension_loans: BTreeMap<BodyNodeId, BTreeSet<LoanId>>,
    loops: Vec<LoopFlow>,
    scopes: Vec<nocter_model::BodyScopeId>,
    closure: Option<(ClosureId, &'program ClosureDefinition)>,
}

impl<'program> Analyzer<'program> {
    fn new(
        facts: ProgramFacts<'program>,
        input: &'program BodyRelationInput<'program>,
        liveness: &'program Liveness,
        closure: Option<(ClosureId, &'program ClosureDefinition)>,
    ) -> Self {
        Self {
            graph: facts.graph,
            types: facts.types,
            capability_evidence: facts.capability_evidence,
            drops: facts.drops,
            provenance: facts.provenance,
            input,
            liveness,
            loans: BTreeMap::new(),
            live_before: HashMap::new(),
            suspension_loans: BTreeMap::new(),
            loops: Vec::new(),
            scopes: Vec::new(),
            closure,
        }
    }

    fn analyze(mut self) -> Result<RootLoanAnalysis, BodyRelationError> {
        let mut state = self.initial_state()?;
        let root = self
            .closure
            .map_or(self.input.body().root(), |(_, definition)| {
                definition.body()
            });
        self.evaluate(root, &mut state, &BTreeSet::new())?;
        let suspension_storage = self.collect_suspension_storage()?;
        Ok(RootLoanAnalysis {
            loans: self.loans,
            live_before: self.live_before,
            suspension_storage,
        })
    }

    fn collect_suspension_storage(
        &self,
    ) -> Result<BTreeMap<BodyNodeId, BTreeSet<SuspensionStorage>>, BodyRelationError> {
        let mut result = BTreeMap::new();
        for (node, live) in &self.suspension_loans {
            let mut storage = BTreeSet::new();
            for loan in live.iter().copied() {
                self.collect_loan_suspension_storage(loan, &mut BTreeSet::new(), &mut storage)?;
            }
            result.insert(*node, storage);
        }
        Ok(result)
    }

    fn collect_loan_suspension_storage(
        &self,
        loan: LoanId,
        visiting: &mut BTreeSet<LoanId>,
        storage: &mut BTreeSet<SuspensionStorage>,
    ) -> Result<(), BodyRelationError> {
        if !visiting.insert(loan) {
            return Ok(());
        }
        let definition = self
            .loans
            .get(&loan)
            .ok_or(BodyCheckInternalError::LoanAnalysis)?;
        for place in definition.places() {
            let LoanRoot::Place(root) = place.root() else {
                continue;
            };
            let stable = match root {
                PlaceRoot::Parameter(parameter) => SuspensionStorage::Parameter(parameter),
                PlaceRoot::Local(local) => SuspensionStorage::Local(local),
                PlaceRoot::Capture(capture) => SuspensionStorage::Capture(capture),
                PlaceRoot::Value(value) => SuspensionStorage::Value(value),
                PlaceRoot::Static(_) => continue,
            };
            storage.insert(stable);
        }
        for parent in definition.parents().iter().copied() {
            self.collect_loan_suspension_storage(parent, visiting, storage)?;
        }
        visiting.remove(&loan);
        Ok(())
    }

    fn initial_state(&mut self) -> Result<LoanState, BodyCheckInternalError> {
        if let Some((closure, definition)) = self.closure {
            return self.initial_closure_state(closure, definition);
        }
        self.initial_declared_state()
    }

    fn initial_closure_state(
        &mut self,
        closure: ClosureId,
        definition: &ClosureDefinition,
    ) -> Result<LoanState, BodyCheckInternalError> {
        let mut state = LoanState::default();
        for (position, parameter) in definition
            .signature()
            .parameters()
            .iter()
            .copied()
            .enumerate()
        {
            let binding = parameter.binding();
            let ty = parameter.ty();
            let Some(TypeKind::Borrow { capability, .. }) = self.types.get(ty) else {
                state.set_root(PlaceRoot::Local(binding), LoanValue::independent());
                continue;
            };
            let loan = LoanId::ClosureParameter {
                closure,
                origin: ParameterOrigin::new(position),
            };
            self.loans.insert(
                loan,
                CheckedLoan::new(
                    *capability,
                    [LoanPlace::new(
                        LoanRoot::External(PlaceRoot::Local(binding)),
                        [],
                    )],
                    [],
                ),
            );
            state.set_root(PlaceRoot::Local(binding), LoanValue::from_loan(loan));
        }
        for capture in definition.environment().iter().copied() {
            let binding = capture.binding();
            let declaration = self
                .input
                .body()
                .captures()
                .get(binding)
                .ok_or(BodyCheckInternalError::LoanAnalysis)?
                .declaration();
            let capability = match declaration.mode() {
                crate::CaptureMode::Readonly => Some(BorrowCapability::Readonly),
                crate::CaptureMode::ReadWrite => Some(BorrowCapability::ReadWrite),
                crate::CaptureMode::Move => None,
            };
            let value = if let Some(capability) = capability {
                let loan = LoanId::ClosureCapture {
                    closure,
                    capture: binding,
                };
                self.loans.insert(
                    loan,
                    CheckedLoan::new(
                        capability,
                        [LoanPlace::new(
                            LoanRoot::External(PlaceRoot::Capture(binding)),
                            [],
                        )],
                        [],
                    ),
                );
                LoanValue::from_loan(loan)
            } else {
                LoanValue::independent()
            };
            state.set_root(PlaceRoot::Capture(binding), value);
        }
        Ok(state)
    }

    fn initial_declared_state(&mut self) -> Result<LoanState, BodyCheckInternalError> {
        let mut state = LoanState::default();
        let BodyOwner::Callable(callable) = self.input.owner() else {
            return Ok(state);
        };
        let callable = self
            .graph
            .declarations()
            .callables()
            .get(callable)
            .ok_or(BodyCheckInternalError::LoanAnalysis)?;
        for parameter in callable
            .receiver()
            .into_iter()
            .chain(callable.parameters().iter().copied())
        {
            let declaration = self
                .graph
                .declarations()
                .parameters()
                .get(parameter)
                .ok_or(BodyCheckInternalError::LoanAnalysis)?;
            let Some(TypeKind::Borrow { capability, .. }) = self.types.get(declaration.ty()) else {
                state.set_root(PlaceRoot::Parameter(parameter), LoanValue::independent());
                continue;
            };
            let loan = LoanId::Parameter(parameter);
            self.loans.insert(
                loan,
                CheckedLoan::new(
                    *capability,
                    [LoanPlace::new(
                        LoanRoot::External(PlaceRoot::Parameter(parameter)),
                        [],
                    )],
                    [],
                ),
            );
            state.set_root(PlaceRoot::Parameter(parameter), LoanValue::from_loan(loan));
        }
        Ok(state)
    }

    fn evaluate(
        &mut self,
        node: BodyNodeId,
        state: &mut LoanState,
        extra_active: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyRelationError> {
        self.record_live(node, state, extra_active);
        let checked = self
            .input
            .body()
            .nodes()
            .get(node)
            .ok_or(BodyCheckInternalError::MissingNode(node))?;
        let ty = checked.ty();
        let operation = checked.operation().clone();
        let mut result = match operation {
            CheckedOperation::Complete
            | CheckedOperation::Constant(_)
            | CheckedOperation::ArgumentPackLength(_) => (LoanValue::independent(), true),
            CheckedOperation::Place(place) => {
                self.evaluate_place_indices(place, state, extra_active)?;
                (self.read_place(place, state)?, true)
            }
            CheckedOperation::Copy(place) => {
                self.evaluate_place_indices(place, state, extra_active)?;
                self.check_place_access(node, place, AccessKind::Read, state, extra_active)?;
                (self.read_place(place, state)?, true)
            }
            CheckedOperation::Move(place) => {
                self.evaluate_place_indices(place, state, extra_active)?;
                self.check_place_access(node, place, AccessKind::Write, state, extra_active)?;
                let value = self.read_place(place, state)?;
                self.remove_place(place, state)?;
                (value, true)
            }
            CheckedOperation::Borrow { capability, place } => {
                self.evaluate_place_indices(place, state, extra_active)?;
                let value = self.issue_loan(node, place, capability, state, extra_active)?;
                (value, true)
            }
            CheckedOperation::BorrowConversion(conversion) => {
                self.evaluate(conversion.value(), state, extra_active)?
            }
            CheckedOperation::Await(await_) => {
                let (computation, reaches) =
                    self.evaluate(await_.computation(), state, extra_active)?;
                let mut suspended = self.live_before.get(&node).cloned().unwrap_or_default();
                suspended.extend(computation.all_loans());
                self.suspension_loans
                    .entry(node)
                    .or_default()
                    .extend(suspended);
                (
                    computation.projected(crate::ProvenanceProjection::AsyncOutput),
                    reaches,
                )
            }
            CheckedOperation::CallableGuaranteeErasure(value) => {
                self.evaluate(value, state, extra_active)?
            }
            CheckedOperation::OpaqueWitness(witness) => {
                self.evaluate(witness.value(), state, extra_active)?
            }
            CheckedOperation::Primitive(operation) => {
                self.evaluate_primitive(&operation, state, extra_active)?
            }
            CheckedOperation::Comparison(comparison) => {
                self.evaluate_comparison(node, &comparison, state, extra_active)?
            }
            CheckedOperation::Call(call) => self.evaluate_call(node, &call, state, extra_active)?,
            CheckedOperation::Aggregate(aggregate) => {
                self.evaluate_aggregate(&aggregate, state, extra_active)?
            }
            CheckedOperation::Outcome(outcome) => {
                self.evaluate_outcome(&outcome, state, extra_active)?
            }
            CheckedOperation::Control(control) => {
                self.evaluate_control(node, &control, state, extra_active)?
            }
            CheckedOperation::StringLiteral { allocation, .. } => {
                self.evaluate_allocation(allocation, state, extra_active)?;
                (LoanValue::independent(), true)
            }
            CheckedOperation::IteratorAcquisition(acquisition) => {
                self.evaluate_iterator_acquisition(node, &acquisition, state, extra_active)?
            }
            CheckedOperation::PackLiteral(sequence) => {
                self.evaluate_pack_literal(&sequence, state, extra_active)?
            }
            CheckedOperation::Interpolation(interpolation) => {
                self.evaluate_interpolation(node, &interpolation, state, extra_active)?
            }
            CheckedOperation::Closure(closure) => {
                self.evaluate_closure(&closure, state, extra_active)?
            }
        };
        if !crate::provenance::type_can_carry_loan(self.graph, self.types, ty) {
            result.0 = LoanValue::independent();
        }
        state.set_node(node, result.0.clone());
        Ok(result)
    }

    fn evaluate_interpolation(
        &mut self,
        owner: BodyNodeId,
        interpolation: &crate::CheckedInterpolation,
        state: &mut LoanState,
        extra_active: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyRelationError> {
        self.evaluate_allocation(interpolation.allocation(), state, extra_active)?;
        for (position, part) in interpolation.parts().iter().enumerate() {
            match part {
                crate::InterpolationPart::Text(_) => {}
                crate::InterpolationPart::Formatted { operand, .. } => {
                    let position = u16::try_from(position)
                        .map_err(|_| BodyCheckInternalError::LoanAnalysis)?;
                    if !self
                        .evaluate_readonly_operand(owner, position, operand, state, extra_active)?
                        .1
                    {
                        return Ok((LoanValue::independent(), false));
                    }
                }
                crate::InterpolationPart::Diverging(value) => {
                    if !self.evaluate(*value, state, extra_active)?.1 {
                        return Ok((LoanValue::independent(), false));
                    }
                    return Err(BodyCheckInternalError::LoanAnalysis.into());
                }
            }
        }
        Ok((LoanValue::independent(), true))
    }

    fn evaluate_iterator_acquisition(
        &mut self,
        node: BodyNodeId,
        acquisition: &crate::CheckedIteratorAcquisition,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyRelationError> {
        let Some(source) = self.evaluate_receiver(node, 0, acquisition.source(), state, extra)?
        else {
            return Ok((LoanValue::independent(), false));
        };
        let result = match acquisition.acquisition() {
            crate::IterationAcquisition::Direct => source.into_carried(),
            crate::IterationAcquisition::Expansion(selection) => match selection.dispatch() {
                crate::StaticDispatch::Direct(callable) => {
                    self.map_callable_result(callable, Some(&source), &[])?
                }
                crate::StaticDispatch::StructuralRequirement { evidence } => {
                    if !matches!(
                        self.capability_evidence
                            .get(evidence)
                            .map(crate::body_check::CapabilityEvidence::predicate),
                        Some(crate::CheckedPredicate::Expansion { .. })
                    ) {
                        return Err(BodyCheckInternalError::LoanAnalysis.into());
                    }
                    source.into_carried()
                }
                crate::StaticDispatch::InterfaceMethod { .. }
                | crate::StaticDispatch::InterfaceSelfMethod { .. }
                | crate::StaticDispatch::InterfaceDefault { .. }
                | crate::StaticDispatch::OpaqueMethod { .. } => {
                    return Err(BodyCheckInternalError::LoanAnalysis.into());
                }
            },
        };
        Ok((result, true))
    }

    fn evaluate_closure(
        &mut self,
        closure: &crate::CheckedClosure,
        state: &mut LoanState,
        extra_active: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyRelationError> {
        let mut value = LoanValue::independent();
        for capture in closure.captures() {
            let (initializer, reaches) =
                self.evaluate(capture.initializer(), state, extra_active)?;
            if !reaches {
                return Ok((LoanValue::independent(), false));
            }
            let checked_initializer = self
                .input
                .body()
                .nodes()
                .get(capture.initializer())
                .ok_or(BodyCheckInternalError::LoanAnalysis)?;
            let captured = match checked_initializer.operation() {
                CheckedOperation::Borrow { place, .. } => self.read_place(*place, state)?,
                _ => initializer.clone(),
            };
            value.insert_projection(
                crate::ProvenanceProjection::ClosureCaptureValue(capture.binding()),
                captured,
            );
            if matches!(
                checked_initializer.operation(),
                CheckedOperation::Borrow { .. }
            ) {
                value.insert_projection(
                    crate::ProvenanceProjection::ClosureCaptureStorage(capture.binding()),
                    initializer,
                );
            }
        }
        Ok((value, true))
    }

    fn active_loans(
        &self,
        node: BodyNodeId,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> BTreeSet<LoanId> {
        let mut active = extra.clone();
        if let Some(live) = self.liveness.before(node) {
            for slot in live {
                active.extend(state.value(slot).all_loans());
            }
        }
        active
    }

    fn record_live(&mut self, node: BodyNodeId, state: &LoanState, extra: &BTreeSet<LoanId>) {
        let active = self.active_loans(node, state, extra);
        self.live_before.entry(node).or_default().extend(active);
    }
}
