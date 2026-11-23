//! Frozen probe recipes refer to the original shared graph and value-source identities.
use super::{Evidence, Failure, InstanceControl};
use crate::value_access::{ExpressionKind, Referent, flow, results};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{CodeInstanceKey, Program};
use nocter_model::{AccessFlowPointId, FieldPath, TypeId, ValueField};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;

#[derive(Clone, Debug)]
struct Probe {
    source: results::ValueSourceId,
    data: TypeId,
    fields: FieldPath,
    independent_root: bool,
}
#[derive(Clone, Debug)]
pub(in crate::value_access::results) struct Original {
    pub(super) graph: Arc<flow::Graph>,
    probes: BTreeMap<AccessFlowPointId, Probe>,
}
impl Original {
    pub(in crate::value_access::results) fn has_probes(&self) -> bool {
        !self.probes.is_empty()
    }
    pub(in crate::value_access::results) fn prepare(
        typed: &crate::value_access::Query,
        graph: Arc<flow::Graph>,
        sources: Option<&results::ValueSourceFlow>,
    ) -> Result<Self, flow::IntegrityError> {
        let mut probes = BTreeMap::new();
        for point in graph.reachable()? {
            let block = graph.points().get(point).ok_or(INVALID)?;
            let selected = if let flow::Terminator::Branch { condition, .. } = block.terminator {
                let expression = typed.body().expressions().get(condition).ok_or(INVALID)?;
                if let ExpressionKind::PrimaryCondition { source } = expression.kind {
                    let action = block
                        .actions
                        .iter()
                        .position(|action| {
                            matches!(action.operation,
                        flow::Operation::Evaluate(id) if id == condition)
                        })
                        .ok_or(INVALID)?;
                    Some((action, flow::OutcomeSource::Referent(source)))
                } else {
                    None
                }
            } else {
                flow::assertion_source(typed, block)?
                    .map(|source| (block.actions.len() - 1, source))
            };
            let Some((action, source)) = selected else {
                continue;
            };
            let sources = sources.ok_or(INVALID)?;
            let at = flow::Location { point, action };
            let probe = match source {
                flow::OutcomeSource::Value(expression) => Probe {
                    source: sources.operand(at, expression).ok_or(INVALID)?,
                    data: source.data(typed)?,
                    fields: FieldPath::default(),
                    independent_root: false,
                },
                flow::OutcomeSource::Referent(referent) => Probe {
                    source: sources.root(at, referent).ok_or(INVALID)?,
                    data: results::values::root_type(typed, referent).ok_or(INVALID)?,
                    fields: match referent {
                        Referent::Place(place) => typed
                            .body()
                            .places()
                            .get(place)
                            .ok_or(INVALID)?
                            .fields
                            .clone(),
                        Referent::Evaluated(_) => FieldPath::default(),
                    },
                    independent_root: true,
                },
            };
            let program = typed.program();
            let outcome = program
                .project_data(probe.data, &probe.fields)
                .ok_or(INVALID)?;
            let primary = program
                .component(outcome, ValueField::Variant(true))
                .ok_or(INVALID)?
                .data;
            // This is only an invariance test. The maximally excluded metadata is never supplied
            // as an actual value, frame input, layout or represented-domain certificate.
            if program
                .instantiate_callable_domains(primary, |_, _| Ok(None))
                .is_ok_and(|minimal| !minimal.is_uninhabited())
            {
                continue;
            }
            probes.insert(point, probe);
        }
        Ok(Self { graph, probes })
    }
    pub(in crate::value_access::results) fn instantiate(
        &self,
        program: &Program,
        evidence: &Evidence,
        frame: &CodeInstanceKey,
        work: &mut WorkBudget,
    ) -> Result<InstanceControl, Failure> {
        let mut points = BTreeMap::new();
        let mut pending = BTreeSet::from([self.graph.entry()]);
        while let Some(point) = pending.pop_first() {
            if points.contains_key(&point) {
                continue;
            }
            work.step().map_err(|_| Failure::WorkLimit)?;
            let block = self.graph.points().get(point).ok_or(Failure::Unresolved)?;
            let mut terminator = block.terminator.clone();
            if let Some(probe) = self.probes.get(&point)
                && probe.excluded(program, evidence, frame, work)?
            {
                terminator = match terminator {
                    flow::Terminator::Branch { no, .. } => flow::Terminator::Jump(no),
                    flow::Terminator::Jump(_) => flow::Terminator::Unreachable,
                    _ => return Err(Failure::Unresolved),
                };
            }
            pending.extend(terminator.successors());
            points.insert(point, terminator);
        }
        Ok(InstanceControl {
            frame: frame.clone(),
            points,
        })
    }
}
impl Probe {
    fn excluded(
        &self,
        program: &Program,
        evidence: &Evidence,
        frame: &CodeInstanceKey,
        work: &mut WorkBudget,
    ) -> Result<bool, Failure> {
        let closed = self
            .independent_root
            .then(|| program.instantiate_closed_data(self.data))
            .and_then(Result::ok);
        let key = match closed.map_or_else(
            || {
                results::instantiate_value_data(
                    program,
                    evidence,
                    frame,
                    self.source,
                    self.data,
                    work,
                )
            },
            Ok,
        ) {
            Ok(key) => key,
            Err(Failure::WorkLimit) => return Err(Failure::WorkLimit),
            Err(_) => return Ok(false),
        };
        Ok(key
            .project_path(program, &self.fields)
            .and_then(|key| key.component(program, ValueField::Variant(true)))
            .map_err(|_| Failure::Unresolved)?
            .is_uninhabited())
    }
}
