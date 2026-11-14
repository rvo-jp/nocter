//! Exact source value paths select occurrence contexts; a source code name cannot replace them.
use super::{Fact, InstantiationFailure, Key, Query};
use crate::value_access::{
    CallValueSources, ValueSource, ValueSourceId, ValueSourceNode, ir::LocalOrigin,
};
use nocter_declarations::value_access::{
    CodeInstanceKey, FunctionWitness, InstanceSchema, PayloadSchema,
};
use nocter_model::{
    FieldPath, IndexRelation, PathRelation, TypeId, ValueApplication, ValueField,
    ValueFunctionOrigin as Origin, ValuePath, ValueProjection,
};

mod data;
type Result = std::result::Result<Fact, InstantiationFailure>;
impl Query<'_, '_> {
    pub(super) fn source_result(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        path: &ValuePath<()>,
    ) -> Result {
        let Some(graph) = self.evidence.value_sources(frame.code()) else {
            return Ok(Fact::Unavailable);
        };
        let values: Vec<_> = graph.returned().iter().copied().collect();
        let mut fact = Fact::Empty;
        for value in values {
            fact = fact.join(self.read(owner, Key::Value(frame.clone(), value, path.clone()))?);
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
        let selected = self.read(
            owner,
            Key::Value(frame.clone(), bundle.callee, ValuePath::default()),
        )?;
        let selected = self.adapt(selected, bundle.invocation);
        let mut pending = matches!(selected, Fact::Empty);
        let mut unavailable = !matches!(selected, Fact::Empty | Fact::Witness(_));
        let mut bindings = Vec::new();
        for (&parameter, &index) in &bundle.parameters {
            let Some(&value) = bundle.arguments.get(index) else {
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
        let Fact::Witness(selected) = selected else {
            return Ok(Fact::Unavailable);
        };
        Ok(self
            .program
            .bind_instance(&selected, bindings)
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
            && self.evidence.unproduced_origin(Origin::Application(site))
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
    fn source_node(
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
            ValueSource::Input { origin, .. } => self.source_input(frame, node.data, origin, path),
            ValueSource::Merge { inputs, .. } | ValueSource::Alternatives(inputs) => {
                let mut fact = Fact::Empty;
                for input in inputs {
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
            ValueSource::Adapt { source } => {
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
                if self.evidence.unproduced_origin(Origin::Application(site)) {
                    Fact::Empty
                } else {
                    match self.read(owner, Key::Call(frame.clone(), site))? {
                        Fact::Frame(callee) => {
                            self.read(owner, Key::Result(callee, path.clone()))?
                        }
                        Fact::Empty => Fact::Empty,
                        _ => Fact::Unavailable,
                    }
                }
            }
            ValueSource::CallWrite {
                call,
                argument: 0,
                source,
                ..
            } if path.is_root()
                && self
                    .evidence
                    .value_sources(frame.code())
                    .and_then(|graph| graph.call(call))
                    .is_some_and(|bundles| {
                        bundles
                            .iter()
                            .all(|bundle| bundle.kind == crate::value_access::CallKind::Value)
                    }) =>
            {
                match self.read(
                    owner,
                    Key::Value(frame.clone(), source, ValuePath::default()),
                )? {
                    Fact::Witness(witness)
                        if self.program.concrete_witness(witness.witness()).as_ref()
                            == Ok(&witness) =>
                    {
                        Fact::Witness(witness)
                    }
                    Fact::Empty => Fact::Empty,
                    _ => Fact::Unavailable,
                }
            }
            ValueSource::Opaque | ValueSource::CallWrite { .. } if path.is_root() => node
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
        origin: LocalOrigin,
        path: &ValuePath<()>,
    ) -> Fact {
        if origin == LocalOrigin::Capture {
            return self
                .program
                .instantiate_environment(frame)
                .ok()
                .flatten()
                .and_then(|key| key.project_path(self.program, &concrete_path(path)).ok())
                .and_then(|key| match key.schema(self.program).ok()? {
                    InstanceSchema::Function(witness) => Some(witness),
                    _ => None,
                })
                .map_or(Fact::Unavailable, Fact::Witness);
        }
        let Some(PayloadSchema::Function(witness)) =
            data.and_then(|data| self.program.data_schema(data))
        else {
            return Fact::Unavailable;
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
    fn source_replacement(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        replacement: (ValueSourceId, ValueSourceId, bool),
        written: &ValuePath<()>,
        selected: &ValuePath<()>,
    ) -> Result {
        let (source, value, weak) = replacement;
        if written.relation(selected, |(), ()| IndexRelation::Unknown) == PathRelation::Disjoint {
            return self.read(owner, Key::Value(frame.clone(), source, selected.clone()));
        }
        let Some(tail) = selected.fields().strip_prefix(written.fields()) else {
            return Ok(Fact::Unavailable);
        };
        let supplied = self.read(
            owner,
            Key::Value(frame.clone(), value, ValuePath::new(tail.iter().copied())),
        )?;
        if weak {
            Ok(supplied
                .join(self.read(owner, Key::Value(frame.clone(), source, selected.clone()))?))
        } else {
            Ok(supplied)
        }
    }
}
fn same_field(field: ValueField, selected: ValueProjection<()>) -> bool {
    match (field, selected) {
        (ValueProjection::Array(_), ValueProjection::Array(())) => true,
        (ValueProjection::Tuple(a), ValueProjection::Tuple(b)) => a == b,
        (ValueProjection::Record(a), ValueProjection::Record(b)) => a == b,
        (ValueProjection::Variant(a), ValueProjection::Variant(b)) => a == b,
        _ => false,
    }
}
fn concrete_path(path: &ValuePath<()>) -> FieldPath {
    FieldPath::new(path.fields().iter().map(|&field| match field {
        ValueProjection::Array(()) => ValueField::Array(0),
        ValueProjection::Tuple(index) => ValueField::Tuple(index),
        ValueProjection::Record(field) => ValueField::Record(field),
        ValueProjection::Variant(primary) => ValueField::Variant(primary),
    }))
}
