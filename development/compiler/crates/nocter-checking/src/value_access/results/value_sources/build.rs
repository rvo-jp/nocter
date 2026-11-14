//! Definitions are allocated once per original action; entry merges close through predecessor IDs.
use super::{
    CallValueSources, INVALID, State, ValueSource, ValueSourceId, ValueSourceNode,
    ValueSourceSlot as Slot, ValueSources,
};
use crate::value_access::{ExpressionKind, Query, Referent, flow};
use nocter_model::{AccessExpressionId, AccessFlowPointId, TypeId};
use std::collections::{BTreeMap, BTreeSet};

mod expressions;
mod operations;
type Result<T> = std::result::Result<T, flow::IntegrityError>;
struct Builder<'a> {
    query: &'a Query,
    origin: Option<flow::Location>,
    aliases: &'a flow::Aliases,
    nodes: Vec<ValueSourceNode>,
    defined: BTreeSet<AccessExpressionId>,
    incoming: BTreeMap<(AccessFlowPointId, Slot), ValueSourceId>,
    pending: Vec<(AccessFlowPointId, Slot)>,
    inputs: State,
    outgoing: BTreeMap<AccessFlowPointId, State>,
    predecessors: BTreeMap<AccessFlowPointId, BTreeSet<AccessFlowPointId>>,
    calls: BTreeMap<AccessExpressionId, Vec<CallValueSources>>,
    creations: BTreeMap<nocter_model::ValueClosure, ValueSourceId>,
    returned: BTreeSet<ValueSourceId>,
}
pub(in crate::value_access::results) fn analyze(
    control: &flow::Control,
    aliases: &flow::Aliases,
) -> Result<ValueSources> {
    let query = &control.typed;
    let mut builder = Builder {
        query,
        origin: None,
        aliases,
        nodes: vec![ValueSourceNode {
            origin: None,
            data: None,
            expression: None,
            value: ValueSource::Empty,
        }],
        defined: BTreeSet::new(),
        incoming: BTreeMap::new(),
        pending: vec![],
        inputs: State::new(),
        outgoing: BTreeMap::new(),
        predecessors: BTreeMap::new(),
        calls: BTreeMap::new(),
        creations: BTreeMap::new(),
        returned: BTreeSet::new(),
    };
    for (local, input) in query
        .body
        .locals
        .iter()
        .filter(|(_, local)| local.origin.initialized_on_entry())
    {
        let id = builder.push(
            Some(input.data),
            None,
            ValueSource::Input {
                local,
                origin: input.origin,
            },
        );
        builder.inputs.insert(Slot::Local(local), id);
    }
    let reached = control.graph.reachable()?;
    builder.definitions(control, &reached)?;
    for &point in &reached {
        let block = control.graph.points().get(point).ok_or(INVALID)?;
        let mut state = State::new();
        for (index, action) in block.actions.iter().enumerate() {
            builder.origin = Some(flow::Location {
                point,
                action: index,
            });
            builder.operation(point, &action.operation, &mut state)?;
        }
        builder.outgoing.insert(point, state);
        for next in block.terminator.successors() {
            builder.predecessors.entry(next).or_default().insert(point);
        }
    }
    builder.origin = None;
    while let Some((point, slot)) = builder.pending.pop() {
        let id = builder.incoming[&(point, slot)];
        let mut inputs = BTreeSet::new();
        if point == control.graph.entry() {
            inputs.insert(
                builder
                    .inputs
                    .get(&slot)
                    .copied()
                    .unwrap_or(ValueSourceId(0)),
            );
        } else {
            let predecessors = builder
                .predecessors
                .get(&point)
                .cloned()
                .unwrap_or_default();
            for previous in predecessors {
                let value = builder
                    .outgoing
                    .get(&previous)
                    .and_then(|state| state.get(&slot))
                    .copied();
                inputs.insert(value.unwrap_or_else(|| builder.entry(previous, slot)));
            }
        }
        builder.nodes[id.0].value = ValueSource::Merge {
            point,
            slot,
            inputs,
        };
    }
    Ok(ValueSources {
        owner: query.code(),
        nodes: builder.nodes.into(),
        calls: builder.calls,
        creations: builder.creations,
        returned: builder.returned,
    })
}
impl Builder<'_> {
    fn definitions(
        &mut self,
        control: &flow::Control,
        reached: &BTreeSet<AccessFlowPointId>,
    ) -> Result<()> {
        for &point in reached {
            let block = control.graph.points().get(point).ok_or(INVALID)?;
            for action in &block.actions {
                let defined = match action.operation {
                    flow::Operation::Evaluate(id) | flow::Operation::CommitCall(id) => Some(id),
                    flow::Operation::Forward { result, .. }
                    | flow::Operation::BooleanAnswer { result, .. }
                    | flow::Operation::BooleanConstant { result, .. } => Some(result),
                    _ => None,
                };
                self.defined.extend(defined);
            }
        }
        Ok(())
    }
    fn push(
        &mut self,
        data: Option<TypeId>,
        expression: Option<AccessExpressionId>,
        value: ValueSource,
    ) -> ValueSourceId {
        let id = ValueSourceId(self.nodes.len());
        self.nodes.push(ValueSourceNode {
            origin: self.origin,
            data,
            expression,
            value,
        });
        id
    }
    fn entry(&mut self, point: AccessFlowPointId, slot: Slot) -> ValueSourceId {
        if let Some(&id) = self.incoming.get(&(point, slot)) {
            return id;
        }
        let id = self.push(None, None, ValueSource::Empty);
        self.nodes[id.0].origin = None;
        self.incoming.insert((point, slot), id);
        self.pending.push((point, slot));
        id
    }
    fn read(&mut self, point: AccessFlowPointId, slot: Slot, state: &State) -> ValueSourceId {
        state
            .get(&slot)
            .copied()
            .unwrap_or_else(|| self.entry(point, slot))
    }
    fn data(&self, expression: AccessExpressionId) -> Result<Option<TypeId>> {
        super::data(self.query, expression)
    }
    fn value(
        &mut self,
        point: AccessFlowPointId,
        expression: AccessExpressionId,
        state: &State,
    ) -> Result<ValueSourceId> {
        match self
            .query
            .body
            .expressions
            .get(expression)
            .ok_or(INVALID)?
            .kind
        {
            ExpressionKind::Place(place) | ExpressionKind::Move(Referent::Place(place)) => {
                self.place(point, Referent::Place(place), state)
            }
            ExpressionKind::Boundary { .. }
            | ExpressionKind::AdaptFunction { .. }
            | ExpressionKind::Move(Referent::Evaluated(_))
                if !state.contains_key(&Slot::Expression(expression))
                    && !self.defined.contains(&expression) =>
            {
                self.expression(point, expression, state)
            }
            _ => Ok(self.read(point, Slot::Expression(expression), state)),
        }
    }
    fn place(
        &mut self,
        point: AccessFlowPointId,
        place: Referent,
        state: &State,
    ) -> Result<ValueSourceId> {
        let paths = self.paths(place)?;
        let mut values = BTreeSet::new();
        for (slot, path) in paths {
            let source = self.read(point, slot, state);
            values.insert(self.push(None, None, ValueSource::Select { source, path }));
        }
        Ok(if values.len() == 1 {
            *values.first().ok_or(INVALID)?
        } else {
            self.push(None, None, ValueSource::Alternatives(values))
        })
    }
}
