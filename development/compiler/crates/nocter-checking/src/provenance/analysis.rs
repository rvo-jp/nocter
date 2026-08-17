mod control;
mod places;
mod values;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use nocter_declarations::{
    BodyOwner, CallableProvenance, CallableProvenanceContract, DeclarationGraph, ProvenanceOrigin,
};
use nocter_model::{ArenaBuilder, BodyId, BodyNodeId, BuiltinType, CallableId, TypeId, TypeStore};

use super::state::ProvenanceState;
use super::{ProvenanceBodyInput, input_for_body};
use crate::{
    AmbientStorageDependence, BodyCheckError, BodyCheckInternalError, BodyRule,
    CallableProvenanceTable, CheckedBody, CheckedBodyProvenance, CheckedOperation,
    ConformanceTable, MethodSelection, PlaceRoot, PrimitiveOperation, ProvenanceSource,
    ProvenanceTable, ValueProvenance,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CallableSummary {
    origins: BTreeSet<ProvenanceOrigin>,
    ambient: AmbientStorageDependence,
}

impl CallableSummary {
    fn from_returned(returned: &ValueProvenance) -> Self {
        let sources = returned.all_sources();
        let origins = sources
            .iter()
            .filter_map(|source| match source {
                ProvenanceSource::Callable(origin) => Some(*origin),
                _ => None,
            })
            .collect();
        let ambient = if sources.contains(&ProvenanceSource::Unknown) {
            AmbientStorageDependence::Unknown
        } else if sources.contains(&ProvenanceSource::CurrentAllocation) {
            AmbientStorageDependence::Current
        } else {
            AmbientStorageDependence::Independent
        };
        Self { origins, ambient }
    }
}

struct BodyAnalysis {
    nodes: nocter_model::Arena<BodyNodeId, ValueProvenance>,
    returned: ValueProvenance,
    return_events: Vec<ReturnEvent>,
}

struct ReturnEvent {
    node: BodyNodeId,
    ty: TypeId,
    value: ValueProvenance,
}

struct LoopFlow {
    id: nocter_model::LoopId,
    breaks: Vec<ProvenanceState>,
    continues: Vec<ProvenanceState>,
}

pub(super) fn analyze_program(
    graph: &DeclarationGraph,
    types: &TypeStore,
    conformances: &ConformanceTable,
    inputs: &[ProvenanceBodyInput<'_, '_>],
) -> Result<ProvenanceTable, BodyCheckError> {
    let mut summaries = initial_summaries(graph);
    loop {
        let previous = summaries.clone();
        for (callable, declaration) in graph.declarations().callables().iter() {
            let Some(body) = declaration.body() else {
                continue;
            };
            let input = input_for_body(inputs, body)
                .ok_or(BodyCheckInternalError::MissingBodySource(body))?;
            let analysis = Analyzer::new(graph, types, &summaries, input).analyze()?;
            let actual = CallableSummary::from_returned(&analysis.returned);
            let effective = match declaration.provenance() {
                CallableProvenanceContract::Inferred => actual,
                CallableProvenanceContract::Declared(contract) => CallableSummary {
                    origins: contract.origins().iter().copied().collect(),
                    ambient: actual.ambient,
                },
            };
            summaries.insert(callable, effective);
        }
        if summaries == previous {
            break;
        }
    }

    let conformance_bounds = conformance_origin_bounds(graph, conformances, &summaries)?;
    let mut bodies = ArenaBuilder::<BodyId, CheckedBodyProvenance>::new();
    for (body, declaration) in graph.declarations().bodies().iter() {
        let input =
            input_for_body(inputs, body).ok_or(BodyCheckInternalError::MissingBodySource(body))?;
        let analysis = Analyzer::new(graph, types, &summaries, input).analyze()?;
        if let BodyOwner::Callable(callable) = declaration.owner() {
            validate_callable_returns(
                graph,
                types,
                input,
                callable,
                &summaries,
                conformance_bounds.get(&callable),
                &analysis,
            )?;
        }
        let actual = bodies.insert(CheckedBodyProvenance::new(
            analysis.nodes,
            analysis.returned,
        ));
        if actual != body {
            return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
        }
    }

    let mut callables = ArenaBuilder::<CallableId, crate::CheckedCallableProvenance>::new();
    for (callable, _) in graph.declarations().callables().iter() {
        let summary = summaries
            .get(&callable)
            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
        let origins = CallableProvenance::from_origins(summary.origins.iter().copied())
            .map_err(|_| BodyCheckInternalError::ProvenanceAnalysis)?;
        let actual = callables.insert(crate::CheckedCallableProvenance::new(
            origins,
            summary.ambient,
        ));
        if actual != callable {
            return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
        }
    }

    Ok(ProvenanceTable::new(
        CallableProvenanceTable::new(callables.finish()),
        bodies.finish(),
    ))
}

fn conformance_origin_bounds(
    graph: &DeclarationGraph,
    conformances: &ConformanceTable,
    summaries: &BTreeMap<CallableId, CallableSummary>,
) -> Result<BTreeMap<CallableId, BTreeSet<ProvenanceOrigin>>, BodyCheckError> {
    let mut bounds = BTreeMap::<CallableId, BTreeSet<ProvenanceOrigin>>::new();
    for (_, conformance) in conformances.entries().iter() {
        for method in conformance.methods() {
            let MethodSelection::Implementation(implementation) = method.selection() else {
                continue;
            };
            let interface = method.interface_method();
            let interface_summary = summaries
                .get(&interface)
                .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
            let mapped =
                map_origin_positions(graph, interface, implementation, &interface_summary.origins)?;
            bounds
                .entry(implementation)
                .and_modify(|current| current.retain(|origin| mapped.contains(origin)))
                .or_insert(mapped);
        }
    }
    Ok(bounds)
}

fn map_origin_positions(
    graph: &DeclarationGraph,
    expected: CallableId,
    actual: CallableId,
    origins: &BTreeSet<ProvenanceOrigin>,
) -> Result<BTreeSet<ProvenanceOrigin>, BodyCheckError> {
    let declarations = graph.declarations().callables();
    let expected = declarations
        .get(expected)
        .ok_or(BodyCheckInternalError::MissingCallable(expected))?;
    let actual = declarations
        .get(actual)
        .ok_or(BodyCheckInternalError::MissingCallable(actual))?;
    origins
        .iter()
        .map(|origin| match origin {
            ProvenanceOrigin::Receiver => actual
                .receiver()
                .map(|_| ProvenanceOrigin::Receiver)
                .ok_or_else(|| BodyCheckInternalError::ProvenanceAnalysis.into()),
            ProvenanceOrigin::Parameter(parameter) => {
                let position = expected
                    .parameters()
                    .iter()
                    .position(|candidate| candidate == parameter)
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                actual
                    .parameters()
                    .get(position)
                    .copied()
                    .map(ProvenanceOrigin::Parameter)
                    .ok_or_else(|| BodyCheckInternalError::ProvenanceAnalysis.into())
            }
        })
        .collect()
}

fn initial_summaries(graph: &DeclarationGraph) -> BTreeMap<CallableId, CallableSummary> {
    graph
        .declarations()
        .callables()
        .iter()
        .map(|(callable, declaration)| {
            let origins = declaration
                .provenance()
                .declared_origins()
                .into_iter()
                .flatten()
                .copied()
                .collect();
            let ambient = if declaration.body().is_none()
                && matches!(
                    declaration.provenance(),
                    CallableProvenanceContract::Inferred
                ) {
                AmbientStorageDependence::Unknown
            } else {
                AmbientStorageDependence::Independent
            };
            (callable, CallableSummary { origins, ambient })
        })
        .collect()
}

fn validate_callable_returns(
    graph: &DeclarationGraph,
    types: &TypeStore,
    input: &ProvenanceBodyInput<'_, '_>,
    callable: CallableId,
    summaries: &BTreeMap<CallableId, CallableSummary>,
    conformance_bound: Option<&BTreeSet<ProvenanceOrigin>>,
    analysis: &BodyAnalysis,
) -> Result<(), BodyCheckError> {
    let declaration = graph
        .declarations()
        .callables()
        .get(callable)
        .ok_or(BodyCheckInternalError::MissingCallable(callable))?;
    let allowed = summaries
        .get(&callable)
        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
    for event in &analysis.return_events {
        if !types.may_carry_storage(event.ty) {
            continue;
        }
        let invalid = event
            .value
            .all_sources()
            .into_iter()
            .any(|source| match source {
                ProvenanceSource::Callable(origin) => {
                    !allowed.origins.contains(&origin)
                        || conformance_bound.is_some_and(|bound| !bound.contains(&origin))
                }
                ProvenanceSource::CurrentAllocation => false,
                ProvenanceSource::Local(_)
                | ProvenanceSource::OwnedParameter(_)
                | ProvenanceSource::Region(_)
                | ProvenanceSource::Temporary(_)
                | ProvenanceSource::Unknown => true,
            });
        if invalid {
            let origin = input
                .origins()
                .get(&event.node)
                .copied()
                .ok_or(BodyCheckInternalError::MissingNodeOrigin(event.node))?;
            let rule = BodyRule::InvalidResultProvenance;
            return Err(BodyCheckError::from_rule(rule, rule.diagnostic(origin)));
        }
    }
    let _ = declaration;
    Ok(())
}

struct Analyzer<'program, 'syntax> {
    graph: &'program DeclarationGraph,
    types: &'program TypeStore,
    summaries: &'program BTreeMap<CallableId, CallableSummary>,
    source: crate::BodySource<'syntax>,
    body: &'program CheckedBody,
    node_values: HashMap<BodyNodeId, ValueProvenance>,
    returned: ValueProvenance,
    return_events: Vec<ReturnEvent>,
    loops: Vec<LoopFlow>,
}

impl<'program, 'syntax> Analyzer<'program, 'syntax> {
    fn new(
        graph: &'program DeclarationGraph,
        types: &'program TypeStore,
        summaries: &'program BTreeMap<CallableId, CallableSummary>,
        input: &ProvenanceBodyInput<'program, 'syntax>,
    ) -> Self {
        Self {
            graph,
            types,
            summaries,
            source: input.source(),
            body: input.body(),
            node_values: HashMap::new(),
            returned: ValueProvenance::independent(),
            return_events: Vec::new(),
            loops: Vec::new(),
        }
    }

    fn analyze(mut self) -> Result<BodyAnalysis, BodyCheckError> {
        let mut state = self.initial_state()?;
        let (root_value, reaches) = self.evaluate(self.body.root(), &mut state)?;
        if reaches {
            self.record_return(self.body.root(), root_value)?;
        }
        let mut nodes = ArenaBuilder::new();
        for (node, _) in self.body.nodes().iter() {
            let actual = nodes.insert(self.node_values.remove(&node).unwrap_or_default());
            if actual != node {
                return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
            }
        }
        if !self.node_values.is_empty() {
            return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
        }
        Ok(BodyAnalysis {
            nodes: nodes.finish(),
            returned: self.returned,
            return_events: self.return_events,
        })
    }

    fn initial_state(&self) -> Result<ProvenanceState, BodyCheckInternalError> {
        let mut state = ProvenanceState::default();
        match self.source.owner() {
            BodyOwner::Callable(callable) => {
                let declaration = self
                    .graph
                    .declarations()
                    .callables()
                    .get(callable)
                    .ok_or(BodyCheckInternalError::MissingCallable(callable))?;
                if let Some(receiver) = declaration.receiver() {
                    state.set_value(
                        PlaceRoot::Parameter(receiver),
                        self.parameter_value(receiver, ProvenanceOrigin::Receiver)?,
                    );
                }
                for parameter in declaration.parameters() {
                    state.set_value(
                        PlaceRoot::Parameter(*parameter),
                        self.parameter_value(*parameter, ProvenanceOrigin::Parameter(*parameter))?,
                    );
                }
            }
            BodyOwner::Drop(drop) => {
                let declaration = self
                    .graph
                    .declarations()
                    .drops()
                    .get(drop)
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                state.set_value(
                    PlaceRoot::Parameter(declaration.receiver()),
                    ValueProvenance::from_source(ProvenanceSource::Unknown),
                );
            }
            BodyOwner::Test(_) => {}
        }
        Ok(state)
    }

    fn parameter_value(
        &self,
        parameter: nocter_model::ParameterId,
        origin: ProvenanceOrigin,
    ) -> Result<ValueProvenance, BodyCheckInternalError> {
        let declaration = self
            .graph
            .declarations()
            .parameters()
            .get(parameter)
            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
        Ok(if self.types.may_carry_storage(declaration.ty()) {
            ValueProvenance::from_source(ProvenanceSource::Callable(origin))
        } else {
            ValueProvenance::independent()
        })
    }

    fn evaluate(
        &mut self,
        node: BodyNodeId,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let checked = self
            .body
            .nodes()
            .get(node)
            .ok_or(BodyCheckInternalError::MissingNode(node))?;
        let ty = checked.ty();
        let operation = checked.operation().clone();
        let result = match operation {
            CheckedOperation::Complete | CheckedOperation::Constant(_) => {
                (ValueProvenance::independent(), true)
            }
            CheckedOperation::Place(place) | CheckedOperation::Copy(place) => {
                self.evaluate_place_indices(place, state)?;
                (self.read_place(place, state)?, true)
            }
            CheckedOperation::Move(place) => {
                self.evaluate_place_indices(place, state)?;
                let value = self.read_place(place, state)?;
                self.remove_place(place, state)?;
                (value, true)
            }
            CheckedOperation::Borrow { place, .. } => {
                self.evaluate_place_indices(place, state)?;
                (self.place_storage(place, state)?, true)
            }
            CheckedOperation::Call(call) => self.evaluate_call(&call, state, ty)?,
            CheckedOperation::BorrowConversion(conversion) => {
                self.evaluate(conversion.value(), state)?
            }
            CheckedOperation::Comparison(comparison) => {
                let (_, left) = self.evaluate(comparison.left().value(), state)?;
                if left {
                    let (_, right) = self.evaluate(comparison.right().value(), state)?;
                    (ValueProvenance::independent(), right)
                } else {
                    (ValueProvenance::independent(), false)
                }
            }
            CheckedOperation::Primitive(operation) => self.evaluate_primitive(&operation, state)?,
            CheckedOperation::Aggregate(aggregate) => self.evaluate_aggregate(&aggregate, state)?,
            CheckedOperation::Outcome(outcome) => self.evaluate_outcome(node, &outcome, state)?,
            CheckedOperation::Control(control) => self.evaluate_control(node, &control, state)?,
            CheckedOperation::StringLiteral { allocation, .. } => {
                (self.allocation_provenance(allocation, state)?, true)
            }
            CheckedOperation::Sequence(sequence) => self.evaluate_sequence(&sequence, state)?,
            CheckedOperation::Interpolation(interpolation) => {
                for part in interpolation.parts() {
                    if let crate::InterpolationPart::Formatted { value, .. } = part {
                        let (_, reaches) = self.evaluate(*value, state)?;
                        if !reaches {
                            return Ok(self.record_node(
                                node,
                                ValueProvenance::independent(),
                                false,
                            ));
                        }
                    }
                }
                (
                    self.allocation_provenance(interpolation.allocation(), state)?,
                    true,
                )
            }
            CheckedOperation::Closure(_) => {
                return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
            }
        };
        let reaches = result.1 && ty != self.types.builtin(BuiltinType::Never);
        Ok(self.record_node(node, result.0, reaches))
    }

    fn record_node(
        &mut self,
        node: BodyNodeId,
        value: ValueProvenance,
        reaches: bool,
    ) -> (ValueProvenance, bool) {
        self.node_values.entry(node).or_default().union_with(&value);
        (value, reaches)
    }

    fn evaluate_primitive(
        &mut self,
        operation: &PrimitiveOperation,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyCheckError> {
        let reaches = match operation {
            PrimitiveOperation::Unary { operand, .. }
            | PrimitiveOperation::IntegerConversion { operand, .. } => {
                self.evaluate(*operand, state)?.1
            }
            PrimitiveOperation::Binary { left, right, .. } => {
                if self.evaluate(*left, state)?.1 {
                    self.evaluate(*right, state)?.1
                } else {
                    false
                }
            }
        };
        Ok((ValueProvenance::independent(), reaches))
    }
}
