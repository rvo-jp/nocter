//! Exact source value paths select occurrence contexts; a source code name cannot replace them.
use super::{Fact, InstantiationFailure, Key, Query, StoredQuery};
use crate::value_access::{
    CallCallee, CallKind, CallValueSources, ValueSource, ValueSourceId, ValueSourceNode,
    ir::LocalOrigin,
};
use nocter_declarations::value_access::{
    CodeInstanceKey, ConcreteWitness, DataInstanceKey, FunctionWitness, InstanceSchema,
    PayloadSchema,
};
use nocter_model::{
    FieldPath, IndexRelation, PathRelation, TypeId, ValueApplication, ValueField,
    ValueFunctionOrigin as Origin, ValueParameter, ValuePath, ValueProjection,
};

mod data;
mod writes;
type Result = std::result::Result<Fact, InstantiationFailure>;
impl Query<'_, '_> {
    fn incoming_values(
        &mut self,
        frame: &CodeInstanceKey,
        node: &ValueSourceNode,
    ) -> std::result::Result<Vec<ValueSourceId>, InstantiationFailure> {
        if let ValueSource::Merge { point, inputs, .. } = &node.value
            && let Some(control) = self.control_for(frame)?
        {
            return Ok(inputs
                .iter()
                .filter(|(boundary, _)| control.edge_reachable(boundary.point, *point))
                .map(|(_, &value)| value)
                .collect());
        }
        Ok(node.value.inputs().collect())
    }
    pub(super) fn source_result(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        path: &ValuePath<()>,
    ) -> Result {
        let Some(graph) = self.evidence.value_sources(frame.code()) else {
            return Ok(Fact::Unavailable);
        };
        if graph.flow().is_some() {
            return self.source_returned(owner, frame, path, StoredQuery::Value);
        }
        let values: Vec<_> = graph.returned().iter().copied().collect();
        let mut fact = Fact::Empty;
        for value in values {
            fact = fact.join(self.read(owner, Key::Value(frame.clone(), value, path.clone()))?);
        }
        Ok(fact)
    }
    fn source_returned(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        path: &ValuePath<()>,
        query: StoredQuery,
    ) -> Result {
        let Some(boundaries) = self
            .evidence
            .value_sources(frame.code())
            .and_then(|graph| graph.flow())
            .map(|flow| flow.completions().collect::<Vec<_>>())
        else {
            return Ok(Fact::Unavailable);
        };
        let mut fact = Fact::Empty;
        for boundary in boundaries {
            if self
                .control_for(frame)?
                .is_some_and(|control| !control.reachable(boundary.point))
            {
                continue;
            }
            let incoming = self.read(
                owner,
                Key::Stored(
                    frame.clone(),
                    crate::value_access::ValueSourceSelection {
                        boundary,
                        slot: crate::value_access::ValueSourceSlot::Return,
                        path: path.clone(),
                    },
                    query,
                ),
            )?;
            fact = self.join_data_facts(fact, incoming)?;
        }
        Ok(fact)
    }
    pub(super) fn source_invocation(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        site: ValueApplication,
    ) -> Result {
        let Some(bundles) = self
            .evidence
            .value_sources(frame.code())
            .and_then(|graph| graph.call(site.call))
            .map(<[_]>::to_vec)
        else {
            return Ok(Fact::Unavailable);
        };
        let mut result = Fact::Empty;
        for bundle in bundles {
            result = result.join(self.source_bundle(owner, frame, &bundle)?);
        }
        Ok(result)
    }
    fn source_bundle(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        bundle: &CallValueSources,
    ) -> Result {
        let callee = match &bundle.callee {
            CallCallee::Witness(source) => *source,
            CallCallee::Generic { code, application } => {
                return Ok(self
                    .program
                    .bind_generic_call(*code, application)
                    .map_or(Fact::Unavailable, Fact::Frame));
            }
        };
        let selected = self.read(
            owner,
            Key::Value(frame.clone(), callee, ValuePath::default()),
        )?;
        let selected = self.adapt(selected, bundle.invocation);
        self.source_bound_bundle(owner, frame, bundle, selected)
    }
    pub(super) fn source_bound_bundle(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        bundle: &CallValueSources,
        selected: Fact,
    ) -> Result {
        let selected = match selected {
            Fact::Witness(selected) => selected,
            Fact::Empty => return Ok(Fact::Empty),
            _ => return Ok(Fact::Unavailable),
        };
        let Ok(parameters) = self.program.callable_input_parameters(selected.code()) else {
            return Ok(Fact::Unavailable);
        };
        let offset = usize::from(bundle.kind == CallKind::Value);
        for (&parameter, &index) in &bundle.parameters {
            if index
                .checked_sub(offset)
                .and_then(|index| parameters.get(&index))
                != Some(&parameter)
            {
                return Ok(Fact::Unavailable);
            }
        }
        let mut bindings = Vec::new();
        let mut pending = false;
        let mut unavailable = false;
        for (index, parameter) in parameters {
            let Some(&value) = index
                .checked_add(offset)
                .and_then(|index| bundle.arguments.get(index))
            else {
                return Ok(Fact::Unavailable);
            };
            match self.read(
                owner,
                Key::Value(frame.clone(), value, ValuePath::default()),
            )? {
                Fact::Witness(witness) => bindings.push((parameter, witness)),
                Fact::Empty => pending = true,
                _ => unavailable = true,
            }
        }
        if unavailable {
            return Ok(Fact::Unavailable);
        }
        if pending {
            return Ok(Fact::Empty);
        }
        self.source_input_bundle(owner, frame, bundle, &selected, bindings)
    }
    fn source_input_bundle(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        bundle: &CallValueSources,
        selected: &ConcreteWitness,
        bindings: Vec<(ValueParameter, ConcreteWitness)>,
    ) -> Result {
        let Ok(contracts) = self.program.compound_input_contracts(selected.witness()) else {
            return Ok(Fact::Unavailable);
        };
        let mut inputs = Vec::new();
        let mut pending = false;
        let mut unavailable = false;
        for (index, data) in contracts {
            let Some(value) = index
                .checked_add(usize::from(bundle.kind == CallKind::Value))
                .and_then(|index| bundle.arguments.get(index))
            else {
                return Ok(Fact::Unavailable);
            };
            let Some(actual) = self
                .source_node(frame, *value, &ValuePath::default())
                .and_then(|node| node.data)
                .filter(|actual| self.program.matches_compound_input(*actual, data))
            else {
                return Ok(Fact::Unavailable);
            };
            match self.read(
                owner,
                Key::Data(frame.clone(), *value, ValuePath::default(), actual),
            )? {
                Fact::Data(supplied) => match supplied.reproject(self.program, data) {
                    Ok(received) => inputs.push((index, received)),
                    Err(_) => unavailable = true,
                },
                Fact::Empty => pending = true,
                _ => unavailable = true,
            }
        }
        if unavailable {
            return Ok(Fact::Unavailable);
        }
        if pending {
            return Ok(Fact::Empty);
        }
        Ok(self
            .program
            .bind_instance_with_inputs(selected, bindings, inputs)
            .map_or(Fact::Unavailable, Fact::Frame))
    }
    pub(super) fn source_value(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        value: ValueSourceId,
        path: &ValuePath<()>,
    ) -> Result {
        let Some(node) = self.source_node(frame, value, path) else {
            return Ok(Fact::Unavailable);
        };
        if let ValueSource::CallResult(site) = node.value
            && self.evidence.uncommitted_application(site)
        {
            return Ok(Fact::Empty);
        }
        // A declaration-closed function type already fixes its code and has no contextual
        // substitutions. This selects metadata only; execution/presence and effects stay checked.
        if matches!(
            node.value,
            ValueSource::Adapt { .. }
                | ValueSource::Select { .. }
                | ValueSource::Index { .. }
                | ValueSource::CallWrite { .. }
                | ValueSource::Opaque
                | ValueSource::CallResult(_)
        ) && let Some(PayloadSchema::Function(witness)) = node
            .data
            .and_then(|data| {
                self.program
                    .project_data_by(data, path, |(), length| length > 0)
            })
            .and_then(|data| self.program.data_schema(data))
            && let Ok(witness) = self.program.concrete_witness(witness)
        {
            return Ok(Fact::Witness(witness));
        }
        let fact = self.source_definition(owner, frame, &node, path)?;
        Ok(if path.is_root() {
            if let Some(PayloadSchema::Function(wanted)) =
                node.data.and_then(|data| self.program.data_schema(data))
            {
                self.adapt(fact, wanted.invocation)
            } else {
                fact
            }
        } else {
            fact
        })
    }
    pub(super) fn source_node(
        &self,
        frame: &CodeInstanceKey,
        value: ValueSourceId,
        path: &ValuePath<()>,
    ) -> Option<ValueSourceNode> {
        let node = self.evidence.value_sources(frame.code())?.node(value)?;
        if let Some(data) = node.data
            && !path.is_root()
            && self
                .program
                .project_data_by(data, path, |(), length| length > 0)
                .is_none()
        {
            return None;
        }
        Some(node.clone())
    }
    fn source_definition(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        node: &ValueSourceNode,
        path: &ValuePath<()>,
    ) -> Result {
        Ok(match node.value.clone() {
            ValueSource::Empty => Fact::Empty,
            ValueSource::Input { local, origin } => {
                self.source_input(frame, node.data, local, origin, path)
            }
            ValueSource::Merge { .. } | ValueSource::Alternatives(_) => {
                let mut fact = Fact::Empty;
                for input in self.incoming_values(frame, node)? {
                    fact = fact
                        .join(self.read(owner, Key::Value(frame.clone(), input, path.clone()))?);
                }
                fact
            }
            ValueSource::Select {
                source,
                path: prefix,
            } => self.read(
                owner,
                Key::Value(frame.clone(), source, prefix.concat(path)),
            )?,
            ValueSource::Adapt { source } | ValueSource::Transport { source } => {
                self.read(owner, Key::Value(frame.clone(), source, path.clone()))?
            }
            ValueSource::Aggregate(fields) => {
                let Some((selected, tail)) = path.fields().split_first() else {
                    return Ok(Fact::Unavailable);
                };
                let mut fact = Fact::Empty;
                for (field, value) in fields {
                    if same_field(field, *selected) {
                        fact = fact.join(self.read(
                            owner,
                            Key::Value(frame.clone(), value, ValuePath::new(tail.iter().copied())),
                        )?);
                    }
                }
                fact
            }
            ValueSource::Repeat { seed } => {
                let Some((ValueProjection::Array(()), tail)) = path.fields().split_first() else {
                    return Ok(Fact::Unavailable);
                };
                self.read(
                    owner,
                    Key::Value(frame.clone(), seed, ValuePath::new(tail.iter().copied())),
                )?
            }
            ValueSource::Index { source, .. } => self.read(
                owner,
                Key::Value(
                    frame.clone(),
                    source,
                    ValuePath::new([ValueProjection::Array(())]).concat(path),
                ),
            )?,
            ValueSource::Replace {
                source,
                value,
                path: written,
                weak,
            } => self.source_replacement(owner, frame, (source, value, weak), &written, path)?,
            ValueSource::Function(_)
            | ValueSource::Closure { .. }
            | ValueSource::CallResult(_)
            | ValueSource::CallWrite { .. }
            | ValueSource::Opaque => {
                return self.source_callable(owner, frame, node, path);
            }
        })
    }
    fn source_callable(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        node: &ValueSourceNode,
        path: &ValuePath<()>,
    ) -> Result {
        Ok(match node.value.clone() {
            ValueSource::Function(witness) if path.is_root() => match witness.origin {
                Origin::Parameter(parameter)
                    if self.program.parameter_owner(parameter) == Some(frame.code()) =>
                {
                    frame
                        .resolve(self.program, witness)
                        .map_or(Fact::Unavailable, Fact::Witness)
                }
                _ => self
                    .program
                    .concrete_witness(witness)
                    .map_or(Fact::Unavailable, Fact::Witness),
            },
            ValueSource::Closure {
                closure,
                environment,
            } if path.is_root() => {
                let Some(schema) = self.program.closure(closure) else {
                    return Ok(Fact::Unavailable);
                };
                let selected = FunctionWitness {
                    origin: Origin::Closure(closure),
                    invocation: schema.contract().invocation,
                };
                match self.read(
                    owner,
                    Key::Data(
                        frame.clone(),
                        environment,
                        ValuePath::default(),
                        schema.environment(),
                    ),
                )? {
                    Fact::Data(environment) => self
                        .program
                        .bind_callable_data(selected, &environment)
                        .map_or(Fact::Unavailable, Fact::Witness),
                    Fact::Empty => Fact::Empty,
                    _ => Fact::Unavailable,
                }
            }
            ValueSource::CallResult(site) => {
                if self.evidence.uncommitted_application(site) {
                    Fact::Empty
                } else {
                    self.source_invocation_result(owner, frame, site, path, None)?
                }
            }
            ValueSource::CallWrite { .. } => {
                return self.source_call_write(owner, frame, node, path);
            }
            ValueSource::Opaque if path.is_root() => node
                .data
                .and_then(|data| match self.program.data_schema(data)? {
                    PayloadSchema::Function(witness) => self.program.concrete_witness(witness).ok(),
                    _ => None,
                })
                .map_or(Fact::Unavailable, Fact::Witness),
            _ => Fact::Unavailable,
        })
    }
    fn source_input(
        &self,
        frame: &CodeInstanceKey,
        data: Option<TypeId>,
        local: nocter_model::AccessLocalId,
        origin: LocalOrigin,
        path: &ValuePath<()>,
    ) -> Fact {
        let witness = match data.and_then(|data| self.program.data_schema(data)) {
            Some(PayloadSchema::Function(witness)) if origin != LocalOrigin::Capture => witness,
            _ => {
                return self
                    .projected_input_data(frame, local, origin, path)
                    .and_then(|key| match key.schema(self.program).ok()? {
                        InstanceSchema::Function(witness) => Some(witness),
                        _ => None,
                    })
                    .map_or(Fact::Unavailable, Fact::Witness);
            }
        };
        if !path.is_root() {
            return Fact::Unavailable;
        }
        match origin {
            LocalOrigin::Parameter(parameter) => frame
                .witnesses()
                .get(&parameter.into())
                .cloned()
                .map_or(Fact::Unavailable, Fact::Witness),
            LocalOrigin::ClosureParameter if matches!(witness.origin, Origin::Parameter(parameter) if self.program.parameter_owner(parameter) == Some(frame.code())) => {
                frame
                    .resolve(self.program, witness)
                    .map_or(Fact::Unavailable, Fact::Witness)
            }
            _ => Fact::Unavailable,
        }
    }
    fn projected_input_data(
        &self,
        frame: &CodeInstanceKey,
        local: nocter_model::AccessLocalId,
        origin: LocalOrigin,
        path: &ValuePath<()>,
    ) -> Option<DataInstanceKey> {
        let root = if origin == LocalOrigin::Capture {
            self.program.instantiate_environment(frame).ok().flatten()
        } else {
            self.input_context(frame, local).cloned()
        }?;
        root.project_path(self.program, &concrete_path(path)).ok()
    }
    fn input_context<'frame>(
        &self,
        frame: &'frame CodeInstanceKey,
        local: nocter_model::AccessLocalId,
    ) -> Option<&'frame DataInstanceKey> {
        let graph = self.evidence.value_sources(frame.code())?;
        let mut matches = graph.explicit_channels()?.iter().enumerate().filter_map(|(index, channel)| {
            let node = graph.node(channel.source)?;
            matches!(node.value, ValueSource::Input { local: actual, origin } if actual == local && origin.is_input() && origin != LocalOrigin::Capture).then_some((index, node))
        });
        let (index, node) = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let data = frame.input_data(index)?;
        (node.data == Some(data.data())).then_some(data)
    }
    fn source_replacement(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        replacement: (ValueSourceId, ValueSourceId, bool),
        written: &ValuePath<()>,
        selected: &ValuePath<()>,
    ) -> Result {
        let (source, value, weak) = replacement;
        let tail = match replacement_path(written, selected) {
            ReplacementPath::Unchanged => {
                return self.read(owner, Key::Value(frame.clone(), source, selected.clone()));
            }
            ReplacementPath::Supplied(tail) => tail,
            ReplacementPath::Components => return Ok(Fact::Unavailable),
        };
        let supplied = self.read(owner, Key::Value(frame.clone(), value, tail))?;
        if weak {
            Ok(supplied
                .join(self.read(owner, Key::Value(frame.clone(), source, selected.clone()))?))
        } else {
            Ok(supplied)
        }
    }
}
enum ReplacementPath {
    Unchanged,
    Supplied(ValuePath<()>),
    Components,
}
fn replacement_path(written: &ValuePath<()>, selected: &ValuePath<()>) -> ReplacementPath {
    if written.relation(selected, |(), ()| IndexRelation::Unknown) == PathRelation::Disjoint {
        return ReplacementPath::Unchanged;
    }
    selected
        .fields()
        .strip_prefix(written.fields())
        .map_or(ReplacementPath::Components, |tail| {
            ReplacementPath::Supplied(ValuePath::new(tail.iter().copied()))
        })
}
fn same_field(field: ValueField, selected: ValueProjection<()>) -> bool {
    field.map_index(|_| ()) == selected
}
fn concrete_path(path: &ValuePath<()>) -> FieldPath {
    FieldPath::new(path.fields().iter().map(|field| field.map_index(|()| 0)))
}
