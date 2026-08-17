mod control;
mod escape;
mod places;
mod values;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use nocter_declarations::{
    BodyOwner, CallableProvenance, CallableProvenanceContract, DeclarationGraph, ProvenanceOrigin,
};
use nocter_model::{
    ArenaBuilder, BodyId, BodyNodeId, BuiltinType, CallableId, CaptureId, ClosureId,
    ParameterOrigin, ResultProvenance, TypeId, TypeStore,
};

use super::state::ProvenanceState;
use super::{ProvenanceBodyInput, input_for_body};
use crate::{
    AmbientStorageDependence, BodyCheckError, BodyCheckInternalError, BodyRule,
    CallableProvenanceTable, CheckedBody, CheckedBodyProvenance, CheckedOperation,
    ClosureProvenanceTable, ClosureTable, ConformanceTable, MethodSelection, PlaceRoot,
    PrimitiveOperation, ProvenanceProjection, ProvenanceSource, ProvenanceTable, ValueProvenance,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CallableSummary {
    origins: BTreeSet<ProvenanceOrigin>,
    ambient: AmbientStorageDependence,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ClosureSummary {
    parameters: BTreeSet<ParameterOrigin>,
    captures: BTreeSet<CaptureId>,
    environment: bool,
    ambient: AmbientStorageDependence,
}

struct ProgramSummaries {
    callables: BTreeMap<CallableId, CallableSummary>,
    closures: BTreeMap<ClosureId, ClosureSummary>,
}

impl ClosureSummary {
    fn from_returned(closure: ClosureId, returned: &ValueProvenance) -> Self {
        let sources = returned.all_sources();
        let parameters = sources
            .iter()
            .filter_map(|source| match source {
                ProvenanceSource::ClosureParameter {
                    closure: actual,
                    origin,
                } if *actual == closure => Some(*origin),
                _ => None,
            })
            .collect();
        let captures = sources
            .iter()
            .filter_map(|source| match source {
                ProvenanceSource::ClosureCaptureValue {
                    closure: actual,
                    capture,
                } if *actual == closure => Some(*capture),
                _ => None,
            })
            .collect();
        let environment = sources.contains(&ProvenanceSource::ClosureEnvironment(closure));
        let ambient = if sources.contains(&ProvenanceSource::Unknown) {
            AmbientStorageDependence::Unknown
        } else if sources.contains(&ProvenanceSource::CurrentAllocation) {
            AmbientStorageDependence::Current
        } else {
            AmbientStorageDependence::Independent
        };
        Self {
            parameters,
            captures,
            environment,
            ambient,
        }
    }
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
    nodes: HashMap<BodyNodeId, ValueProvenance>,
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
    closures: &ClosureTable,
    inputs: &[ProvenanceBodyInput<'_, '_>],
) -> Result<ProvenanceTable, BodyCheckError> {
    let summaries = infer_program_summaries(graph, types, closures, inputs)?;
    let conformance_bounds = conformance_origin_bounds(graph, conformances, &summaries.callables)?;
    let bodies = build_body_provenance(
        graph,
        types,
        closures,
        inputs,
        &summaries.callables,
        &summaries.closures,
        &conformance_bounds,
    )?;
    let callables = build_callable_provenance(graph, &summaries.callables)?;
    let checked_closures = build_closure_provenance(closures, &summaries.closures)?;
    Ok(ProvenanceTable::new(
        CallableProvenanceTable::new(callables),
        ClosureProvenanceTable::new(checked_closures),
        bodies,
    ))
}

fn infer_program_summaries(
    graph: &DeclarationGraph,
    types: &TypeStore,
    closures: &ClosureTable,
    inputs: &[ProvenanceBodyInput<'_, '_>],
) -> Result<ProgramSummaries, BodyCheckError> {
    let mut summaries = initial_summaries(graph);
    let mut closure_summaries = closures
        .definitions()
        .iter()
        .map(|(closure, _)| (closure, ClosureSummary::default()))
        .collect::<BTreeMap<_, _>>();
    loop {
        let previous = summaries.clone();
        let previous_closures = closure_summaries.clone();
        for (callable, declaration) in graph.declarations().callables().iter() {
            let Some(body) = declaration.body() else {
                continue;
            };
            let input = input_for_body(inputs, body)
                .ok_or(BodyCheckInternalError::MissingBodySource(body))?;
            let analysis =
                Analyzer::new_declared(graph, types, &summaries, &closure_summaries, input)
                    .analyze()?;
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
        for (closure, definition) in closures.definitions().iter() {
            let input = input_for_body(inputs, definition.owner()).ok_or(
                BodyCheckInternalError::MissingBodySource(definition.owner()),
            )?;
            let analysis = Analyzer::new_closure(
                graph,
                types,
                &summaries,
                &closure_summaries,
                input,
                closure,
                definition,
            )
            .analyze()?;
            closure_summaries.insert(
                closure,
                ClosureSummary::from_returned(closure, &analysis.returned),
            );
        }
        if summaries == previous && closure_summaries == previous_closures {
            return Ok(ProgramSummaries {
                callables: summaries,
                closures: closure_summaries,
            });
        }
    }
}

fn build_body_provenance(
    graph: &DeclarationGraph,
    types: &TypeStore,
    closures: &ClosureTable,
    inputs: &[ProvenanceBodyInput<'_, '_>],
    summaries: &BTreeMap<CallableId, CallableSummary>,
    closure_summaries: &BTreeMap<ClosureId, ClosureSummary>,
    conformance_bounds: &BTreeMap<CallableId, BTreeSet<ProvenanceOrigin>>,
) -> Result<nocter_model::Arena<BodyId, CheckedBodyProvenance>, BodyCheckError> {
    let mut bodies = ArenaBuilder::<BodyId, CheckedBodyProvenance>::new();
    for (body, declaration) in graph.declarations().bodies().iter() {
        let input =
            input_for_body(inputs, body).ok_or(BodyCheckInternalError::MissingBodySource(body))?;
        let mut analysis =
            Analyzer::new_declared(graph, types, summaries, closure_summaries, input).analyze()?;
        if let BodyOwner::Callable(callable) = declaration.owner() {
            validate_callable_returns(
                graph,
                types,
                input,
                callable,
                summaries,
                conformance_bounds.get(&callable),
                &analysis,
            )?;
        }
        for (closure, definition) in closures
            .definitions()
            .iter()
            .filter(|(_, definition)| definition.owner() == body)
        {
            let closure_analysis = Analyzer::new_closure(
                graph,
                types,
                summaries,
                closure_summaries,
                input,
                closure,
                definition,
            )
            .analyze()?;
            validate_closure_returns(types, input, closure, definition, &closure_analysis)?;
            for (node, value) in closure_analysis.nodes {
                if analysis.nodes.insert(node, value).is_some() {
                    return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                }
            }
        }
        let mut nodes = ArenaBuilder::new();
        for (node, _) in input.body().nodes().iter() {
            let actual = nodes.insert(analysis.nodes.remove(&node).unwrap_or_default());
            if actual != node {
                return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
            }
        }
        if !analysis.nodes.is_empty() {
            return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
        }
        let actual = bodies.insert(CheckedBodyProvenance::new(
            nodes.finish(),
            analysis.returned,
        ));
        if actual != body {
            return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
        }
    }
    Ok(bodies.finish())
}

fn build_callable_provenance(
    graph: &DeclarationGraph,
    summaries: &BTreeMap<CallableId, CallableSummary>,
) -> Result<nocter_model::Arena<CallableId, crate::CheckedCallableProvenance>, BodyCheckError> {
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
    Ok(callables.finish())
}

fn build_closure_provenance(
    closures: &ClosureTable,
    closure_summaries: &BTreeMap<ClosureId, ClosureSummary>,
) -> Result<nocter_model::Arena<ClosureId, crate::CheckedClosureProvenance>, BodyCheckError> {
    let mut checked_closures = ArenaBuilder::<ClosureId, crate::CheckedClosureProvenance>::new();
    for (closure, _) in closures.definitions().iter() {
        let summary = closure_summaries
            .get(&closure)
            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
        let parameters = ResultProvenance::from_origins(summary.parameters.iter().copied())
            .map_err(|_| BodyCheckInternalError::ProvenanceAnalysis)?;
        let actual = checked_closures.insert(crate::CheckedClosureProvenance::new(
            parameters,
            summary.captures.iter().copied().collect::<Vec<_>>(),
            summary.environment,
            summary.ambient,
        ));
        if actual != closure {
            return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
        }
    }
    Ok(checked_closures.finish())
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
                | ProvenanceSource::ClosureParameter { .. }
                | ProvenanceSource::ClosureCaptureValue { .. }
                | ProvenanceSource::ClosureEnvironment(_)
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

fn validate_closure_returns(
    types: &TypeStore,
    input: &ProvenanceBodyInput<'_, '_>,
    closure: ClosureId,
    definition: &crate::ClosureDefinition,
    analysis: &BodyAnalysis,
) -> Result<(), BodyCheckError> {
    for event in &analysis.return_events {
        if !types.may_carry_storage(event.ty) {
            continue;
        }
        let invalid = event
            .value
            .all_sources()
            .into_iter()
            .any(|source| match source {
                ProvenanceSource::ClosureParameter {
                    closure: actual,
                    origin,
                } => {
                    actual != closure
                        || definition
                            .callable_requirements()
                            .iter()
                            .any(|contract| !contract.provenance().origins().contains(&origin))
                }
                ProvenanceSource::ClosureCaptureValue {
                    closure: actual, ..
                }
                | ProvenanceSource::ClosureEnvironment(actual) => actual != closure,
                ProvenanceSource::CurrentAllocation => false,
                ProvenanceSource::Callable(_)
                | ProvenanceSource::Local(_)
                | ProvenanceSource::OwnedParameter(_)
                | ProvenanceSource::Region(_)
                | ProvenanceSource::Temporary(_)
                | ProvenanceSource::Unknown => true,
            });
        if invalid || event.ty != definition.signature().result() {
            let origin = input
                .origins()
                .get(&event.node)
                .copied()
                .ok_or(BodyCheckInternalError::MissingNodeOrigin(event.node))?;
            let rule = BodyRule::InvalidResultProvenance;
            return Err(BodyCheckError::from_rule(rule, rule.diagnostic(origin)));
        }
    }
    Ok(())
}

struct Analyzer<'program, 'syntax> {
    graph: &'program DeclarationGraph,
    types: &'program TypeStore,
    summaries: &'program BTreeMap<CallableId, CallableSummary>,
    closure_summaries: &'program BTreeMap<ClosureId, ClosureSummary>,
    source: crate::BodySource<'syntax>,
    body: &'program CheckedBody,
    origins: &'program HashMap<BodyNodeId, nocter_source_index::SourceOrigin>,
    node_values: HashMap<BodyNodeId, ValueProvenance>,
    returned: ValueProvenance,
    return_events: Vec<ReturnEvent>,
    loops: Vec<LoopFlow>,
    root: BodyNodeId,
    result_type: TypeId,
    closure: Option<(ClosureId, &'program crate::ClosureDefinition)>,
}

impl<'program, 'syntax> Analyzer<'program, 'syntax> {
    fn new_declared(
        graph: &'program DeclarationGraph,
        types: &'program TypeStore,
        summaries: &'program BTreeMap<CallableId, CallableSummary>,
        closure_summaries: &'program BTreeMap<ClosureId, ClosureSummary>,
        input: &ProvenanceBodyInput<'program, 'syntax>,
    ) -> Self {
        let result_type = match input.source().owner() {
            BodyOwner::Callable(callable) => {
                graph.declarations().callables().get(callable).map_or_else(
                    || types.builtin(BuiltinType::Void),
                    nocter_declarations::CallableDeclaration::result,
                )
            }
            BodyOwner::Drop(_) | BodyOwner::Test(_) => types.builtin(BuiltinType::Void),
        };
        Self {
            graph,
            types,
            summaries,
            closure_summaries,
            source: input.source(),
            body: input.body(),
            origins: input.origins(),
            node_values: HashMap::new(),
            returned: ValueProvenance::independent(),
            return_events: Vec::new(),
            loops: Vec::new(),
            root: input.body().root(),
            result_type,
            closure: None,
        }
    }

    fn new_closure(
        graph: &'program DeclarationGraph,
        types: &'program TypeStore,
        summaries: &'program BTreeMap<CallableId, CallableSummary>,
        closure_summaries: &'program BTreeMap<ClosureId, ClosureSummary>,
        input: &ProvenanceBodyInput<'program, 'syntax>,
        closure: ClosureId,
        definition: &'program crate::ClosureDefinition,
    ) -> Self {
        let mut analyzer = Self::new_declared(graph, types, summaries, closure_summaries, input);
        analyzer.root = definition.body();
        analyzer.result_type = definition.signature().result();
        analyzer.closure = Some((closure, definition));
        analyzer
    }

    fn analyze(mut self) -> Result<BodyAnalysis, BodyCheckError> {
        let mut state = self.initial_state()?;
        let (root_value, reaches) = self.evaluate(self.root, &mut state)?;
        if reaches {
            self.record_return(self.root, root_value);
        }
        Ok(BodyAnalysis {
            nodes: self.node_values,
            returned: self.returned,
            return_events: self.return_events,
        })
    }

    fn initial_state(&self) -> Result<ProvenanceState, BodyCheckInternalError> {
        let mut state = ProvenanceState::default();
        if let Some((closure, definition)) = self.closure {
            for (position, (binding, ty)) in definition
                .parameters()
                .iter()
                .copied()
                .zip(definition.signature().parameters().iter().copied())
                .enumerate()
            {
                let value = if self.types.may_carry_storage(ty) {
                    ValueProvenance::from_source(ProvenanceSource::ClosureParameter {
                        closure,
                        origin: ParameterOrigin::new(position),
                    })
                } else {
                    ValueProvenance::independent()
                };
                state.set_value(PlaceRoot::Local(binding), value);
            }
            for capture in definition.environment().iter().copied() {
                let binding = capture.binding();
                let checked = self
                    .body
                    .captures()
                    .get(binding)
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                let value = if self.types.may_carry_storage(checked.ty()) {
                    ValueProvenance::from_source(ProvenanceSource::ClosureCaptureValue {
                        closure,
                        capture: binding,
                    })
                } else {
                    ValueProvenance::independent()
                };
                state.set_value(PlaceRoot::Capture(binding), value);
            }
            return Ok(state);
        }
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
            CheckedOperation::Complete
            | CheckedOperation::Constant(_)
            | CheckedOperation::LiteralPackLength(_) => (ValueProvenance::independent(), true),
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
            CheckedOperation::OpaqueWitness(witness) => self.evaluate(witness.value(), state)?,
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
            CheckedOperation::IteratorAcquisition(acquisition) => {
                self.evaluate_iterator_acquisition(&acquisition, state)?
            }
            CheckedOperation::Sequence(sequence) => self.evaluate_sequence(&sequence, state)?,
            CheckedOperation::Interpolation(interpolation) => {
                let allocation = self.allocation_provenance(interpolation.allocation(), state)?;
                for part in interpolation.parts() {
                    let value = match part {
                        crate::InterpolationPart::Text(_) => continue,
                        crate::InterpolationPart::Formatted { operand, .. } => operand.value(),
                        crate::InterpolationPart::Diverging(value) => *value,
                    };
                    let (_, reaches) = self.evaluate(value, state)?;
                    if !reaches {
                        return Ok(self.record_node(node, ValueProvenance::independent(), false));
                    }
                }
                (allocation, true)
            }
            CheckedOperation::Closure(closure) => {
                let mut value = ValueProvenance::independent();
                for capture in closure.captures() {
                    let (initializer, reaches) = self.evaluate(capture.initializer(), state)?;
                    if !reaches {
                        return Ok(self.record_node(node, ValueProvenance::independent(), false));
                    }
                    let checked_initializer = self
                        .body
                        .nodes()
                        .get(capture.initializer())
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    let captured = match checked_initializer.operation() {
                        CheckedOperation::Borrow { place, .. } => self.read_place(*place, state)?,
                        _ => initializer.clone(),
                    };
                    value.insert_projection(
                        ProvenanceProjection::ClosureCaptureValue(capture.binding()),
                        captured,
                    );
                    if matches!(
                        checked_initializer.operation(),
                        CheckedOperation::Borrow { .. }
                    ) {
                        value.insert_projection(
                            ProvenanceProjection::ClosureCaptureStorage(capture.binding()),
                            initializer,
                        );
                    }
                }
                (value, true)
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
