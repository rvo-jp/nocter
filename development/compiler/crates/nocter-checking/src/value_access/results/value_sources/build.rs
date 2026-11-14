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
    writes: BTreeMap<(AccessFlowPointId, Slot), Vec<(usize, ValueSourceId)>>,
    reads: BTreeMap<(flow::Location, AccessExpressionId), ValueSourceId>,
    referents: BTreeMap<(flow::Location, Referent), ValueSourceId>,
    roots: BTreeMap<(flow::Location, Referent), super::positions::RootSources>,
    origin: Option<flow::Location>,
    aliases: &'a flow::Aliases,
    nodes: Vec<ValueSourceNode>,
    defined: BTreeSet<AccessExpressionId>,
    incoming: BTreeMap<(AccessFlowPointId, Slot), ValueSourceId>,
    pending: Vec<(AccessFlowPointId, Slot)>,
    inputs: State,
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
        writes: BTreeMap::new(),
        reads: BTreeMap::new(),
        referents: BTreeMap::new(),
        roots: BTreeMap::new(),
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
        for next in block.terminator.successors() {
            builder.predecessors.entry(next).or_default().insert(point);
        }
    }
    builder.origin = None;
    let source_flow = builder.source_flow(control, &reached)?;
    builder.close_incoming(&source_flow)?;
    Ok(ValueSources {
        owner: query.code(),
        nodes: builder.nodes.into(),
        calls: builder.calls,
        creations: builder.creations,
        returned: builder.returned,
        flow: Some(source_flow),
    })
}
impl Builder<'_> {
    fn source_flow(
        &mut self,
        control: &flow::Control,
        reached: &BTreeSet<AccessFlowPointId>,
    ) -> Result<super::ValueSourceFlow> {
        Ok(super::ValueSourceFlow {
            entry: control.graph.entry(),
            inputs: std::mem::take(&mut self.inputs),
            points: reached
                .iter()
                .map(|&point| {
                    Ok((
                        point,
                        control
                            .graph
                            .points()
                            .get(point)
                            .ok_or(INVALID)?
                            .actions
                            .len(),
                    ))
                })
                .collect::<Result<_>>()?,
            predecessors: std::mem::take(&mut self.predecessors)
                .into_iter()
                .map(|(point, previous)| (point, previous.into_iter().collect()))
                .collect(),
            writes: std::mem::take(&mut self.writes),
            reads: std::mem::take(&mut self.reads),
            referents: std::mem::take(&mut self.referents),
            roots: std::mem::take(&mut self.roots),
            returns: reached
                .iter()
                .copied()
                .filter(|&point| {
                    control
                        .graph
                        .points()
                        .get(point)
                        .is_some_and(|block| matches!(block.terminator, flow::Terminator::Return))
                })
                .collect(),
        })
    }
    fn close_incoming(&mut self, positions: &super::ValueSourceFlow) -> Result<()> {
        use super::{ValueSourceBoundary, ValueSourceRead};
        while let Some((point, slot)) = self.pending.pop() {
            let id = self.incoming[&(point, slot)];
            let mut inputs = BTreeSet::new();
            let boundary = ValueSourceBoundary { point, action: 0 };
            match positions.read(boundary, slot).ok_or(INVALID)? {
                ValueSourceRead::Empty => {
                    inputs.insert(ValueSourceId(0));
                }
                ValueSourceRead::Value(value) => {
                    inputs.insert(value);
                }
                ValueSourceRead::Incoming(previous) => {
                    for previous in previous {
                        let value = match positions.read(previous, slot).ok_or(INVALID)? {
                            ValueSourceRead::Empty => ValueSourceId(0),
                            ValueSourceRead::Value(value) => value,
                            ValueSourceRead::Incoming(_) => self.entry(previous.point, slot),
                        };
                        inputs.insert(value);
                    }
                }
            }
            self.nodes[id.0].value = ValueSource::Merge {
                point,
                slot,
                inputs,
            };
        }
        Ok(())
    }
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
    fn define(&mut self, slot: Slot, value: ValueSourceId, state: &mut State) -> Result<()> {
        let origin = self.origin.ok_or(INVALID)?;
        self.writes
            .entry((origin.point, slot))
            .or_default()
            .push((origin.action, value));
        state.insert(slot, value);
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
        let value = match self
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
            | ExpressionKind::Aggregate { .. }
            | ExpressionKind::AdaptFunction { .. }
            | ExpressionKind::Move(Referent::Evaluated(_))
                if !state.contains_key(&Slot::Expression(expression))
                    && !self.defined.contains(&expression) =>
            {
                self.expression(point, expression, state)
            }
            _ => Ok(self.read(point, Slot::Expression(expression), state)),
        }?;
        let origin = self.origin.ok_or(INVALID)?;
        self.reads.insert((origin, expression), value);
        Ok(value)
    }
    fn place(
        &mut self,
        point: AccessFlowPointId,
        place: Referent,
        state: &State,
    ) -> Result<ValueSourceId> {
        let (value, root) = self.select_place(point, place, state)?;
        self.roots.insert(
            (self.origin.ok_or(INVALID)?, place),
            super::positions::RootSources {
                before: root,
                after: None,
            },
        );
        Ok(value)
    }
    fn place_after(
        &mut self,
        point: AccessFlowPointId,
        place: Referent,
        state: &State,
    ) -> Result<()> {
        let (_, root) = self.select_place(point, place, state)?;
        self.roots
            .get_mut(&(self.origin.ok_or(INVALID)?, place))
            .ok_or(INVALID)?
            .after = Some(root);
        Ok(())
    }
    fn select_place(
        &mut self,
        point: AccessFlowPointId,
        place: Referent,
        state: &State,
    ) -> Result<(ValueSourceId, super::positions::RootSelection)> {
        let paths = self.paths(place)?;
        let suffix = match place {
            Referent::Place(id) => crate::value_access::paths::known_by(
                &self.query.body.places.get(id).ok_or(INVALID)?.fields,
                |_| (),
            ),
            Referent::Evaluated(_) => nocter_model::ValuePath::default(),
        };
        let mut values = BTreeSet::new();
        let mut roots = BTreeSet::new();
        let mut carriers = BTreeSet::new();
        for (slot, path) in paths {
            let source = self.read(point, slot, state);
            let prefix = path
                .fields()
                .len()
                .checked_sub(suffix.fields().len())
                .ok_or(INVALID)?;
            if !path.fields().ends_with(suffix.fields()) {
                return Err(INVALID);
            }
            let root_path = nocter_model::ValuePath::new(path.fields()[..prefix].iter().copied());
            carriers.insert(super::ValueSourceCarrier {
                slot,
                source,
                path: root_path.clone(),
            });
            let root = (!suffix.is_root()).then(|| {
                self.push(
                    None,
                    None,
                    ValueSource::Select {
                        source,
                        path: root_path,
                    },
                )
            });
            let selected = self.push(None, None, ValueSource::Select { source, path });
            values.insert(selected);
            roots.insert(root.unwrap_or(selected));
        }
        let root = if roots.len() == 1 {
            *roots.first().ok_or(INVALID)?
        } else {
            self.push(None, None, ValueSource::Alternatives(roots))
        };
        let value = if values.len() == 1 {
            *values.first().ok_or(INVALID)?
        } else {
            self.push(None, None, ValueSource::Alternatives(values))
        };
        Ok((
            value,
            super::positions::RootSelection {
                source: root,
                carriers: carriers.into_iter().collect(),
            },
        ))
    }
}
