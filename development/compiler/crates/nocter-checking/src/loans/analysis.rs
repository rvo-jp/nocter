use std::collections::{BTreeMap, BTreeSet, HashMap};

use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_model::{
    ArenaBuilder, BodyId, BodyNodeId, BorrowCapability, LoopId, TypeKind, TypeStore,
};
use nocter_source_index::SourceOrigin;

mod access;
mod calls;
mod cleanup;
mod control;
mod values;

use super::liveness::Liveness;
use super::state::LoanState;
use super::value::LoanValue;
use crate::{
    BodyCheckError, BodyCheckInternalError, BodySource, CheckedBody, CheckedBodyLoans, CheckedLoan,
    CheckedOperation, DropTable, LoanId, LoanPlace, LoanRoot, LoanTable, PlaceRoot,
    ProvenanceTable,
};

pub(super) struct LoanBodyInput<'program, 'syntax> {
    source: BodySource<'syntax>,
    body: &'program CheckedBody,
    origins: &'program HashMap<BodyNodeId, SourceOrigin>,
}

impl<'program, 'syntax> LoanBodyInput<'program, 'syntax> {
    pub(super) const fn new(
        source: BodySource<'syntax>,
        body: &'program CheckedBody,
        origins: &'program HashMap<BodyNodeId, SourceOrigin>,
    ) -> Self {
        Self {
            source,
            body,
            origins,
        }
    }
}

pub(super) fn analyze_program(
    graph: &DeclarationGraph,
    types: &TypeStore,
    drops: &DropTable,
    provenance: &ProvenanceTable,
    inputs: &[LoanBodyInput<'_, '_>],
) -> Result<LoanTable, BodyCheckError> {
    let mut bodies = ArenaBuilder::<BodyId, CheckedBodyLoans>::new();
    for (body, _) in graph.declarations().bodies().iter() {
        let input = inputs
            .iter()
            .find(|input| input.source.body() == body)
            .ok_or(BodyCheckInternalError::MissingBodySource(body))?;
        let liveness = super::liveness::analyze(types, drops, input.body)?;
        let checked = Analyzer::new(graph, types, drops, provenance, input, &liveness).analyze()?;
        let actual = bodies.insert(checked);
        if actual != body {
            return Err(BodyCheckInternalError::LoanAnalysis.into());
        }
    }
    Ok(LoanTable::new(bodies.finish()))
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

struct Analyzer<'program, 'syntax> {
    graph: &'program DeclarationGraph,
    types: &'program TypeStore,
    drops: &'program DropTable,
    provenance: &'program ProvenanceTable,
    input: &'program LoanBodyInput<'program, 'syntax>,
    liveness: &'program Liveness,
    loans: BTreeMap<LoanId, CheckedLoan>,
    live_before: HashMap<BodyNodeId, BTreeSet<LoanId>>,
    loops: Vec<LoopFlow>,
    scopes: Vec<nocter_model::BodyScopeId>,
}

impl<'program, 'syntax> Analyzer<'program, 'syntax> {
    fn new(
        graph: &'program DeclarationGraph,
        types: &'program TypeStore,
        drops: &'program DropTable,
        provenance: &'program ProvenanceTable,
        input: &'program LoanBodyInput<'program, 'syntax>,
        liveness: &'program Liveness,
    ) -> Self {
        Self {
            graph,
            types,
            drops,
            provenance,
            input,
            liveness,
            loans: BTreeMap::new(),
            live_before: HashMap::new(),
            loops: Vec::new(),
            scopes: Vec::new(),
        }
    }

    fn analyze(mut self) -> Result<CheckedBodyLoans, BodyCheckError> {
        let mut state = self.initial_state()?;
        self.evaluate(self.input.body.root(), &mut state, &BTreeSet::new())?;
        let mut live_before = ArenaBuilder::new();
        for (node, _) in self.input.body.nodes().iter() {
            let loans = self
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
        Ok(CheckedBodyLoans::new(self.loans, live_before.finish()))
    }

    fn initial_state(&mut self) -> Result<LoanState, BodyCheckInternalError> {
        let mut state = LoanState::default();
        let BodyOwner::Callable(callable) = self.input.source.owner() else {
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
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        self.record_live(node, state, extra_active);
        let checked = self
            .input
            .body
            .nodes()
            .get(node)
            .ok_or(BodyCheckInternalError::MissingNode(node))?;
        let operation = checked.operation().clone();
        let result = match operation {
            CheckedOperation::Complete | CheckedOperation::Constant(_) => {
                (LoanValue::independent(), true)
            }
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
            CheckedOperation::Sequence(sequence) => {
                self.evaluate_sequence(&sequence, state, extra_active)?
            }
            CheckedOperation::Interpolation(interpolation) => {
                for part in interpolation.parts() {
                    if let crate::InterpolationPart::Formatted { value, .. } = part
                        && !self.evaluate(*value, state, extra_active)?.1
                    {
                        return Ok((LoanValue::independent(), false));
                    }
                }
                self.evaluate_allocation(interpolation.allocation(), state, extra_active)?;
                (LoanValue::independent(), true)
            }
            CheckedOperation::Closure(_) => {
                return Err(BodyCheckInternalError::LoanAnalysis.into());
            }
        };
        state.set_node(node, result.0.clone());
        Ok(result)
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
