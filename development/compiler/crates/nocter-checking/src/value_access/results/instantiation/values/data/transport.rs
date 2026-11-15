//! Returned carriers and sparse replacement paths retain the same canonical component keys.
use super::super::{ReplacementPath, replacement_path};
use super::{
    CodeInstanceKey, Fact, Key, PayloadSchema, Query, Result, TypeId, ValueField, ValuePath,
    ValueProjection, ValueSource, ValueSourceId,
};

impl Query<'_, '_> {
    pub(super) fn transported_data(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        value: ValueSourceId,
        source: ValueSourceId,
        path: &ValuePath<()>,
        target: TypeId,
    ) -> Result {
        let root = ValuePath::default();
        let Some(actual) = self
            .source_node(frame, source, &root)
            .and_then(|node| node.data)
        else {
            return Ok(Fact::Unavailable);
        };
        let Some(destination) = self
            .source_node(frame, value, &root)
            .and_then(|node| node.data)
        else {
            return Ok(Fact::Unavailable);
        };
        let from = match self.read(owner, Key::Data(frame.clone(), source, root, actual))? {
            Fact::Data(key) => key,
            other => return Ok(other),
        };
        let destination = match self
            .evidence
            .closed_domain(self.program, destination)
            .or_else(|error| {
                if self.program.matches_compound_input(actual, destination)
                    || self.evidence.excludes_primary(self.program, destination)
                {
                    from.reproject(self.program, destination)
                } else {
                    Err(error)
                }
            }) {
            Ok(key) => key,
            Err(_) => match self.read(
                owner,
                Key::Representation(frame.clone(), value, ValuePath::default(), destination),
            )? {
                Fact::Data(key) => key,
                other => return Ok(other),
            },
        };
        if !from.permits_guarded_transport(self.program, &destination) {
            return Ok(Fact::Unavailable);
        }
        Ok(destination
            .project_path(self.program, &super::super::concrete_path(path))
            .and_then(|key| key.reproject(self.program, target))
            .map_or(Fact::Unavailable, Fact::Data))
    }
    pub(in super::super::super) fn source_completed_input(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        local: nocter_model::AccessLocalId,
        path: &ValuePath<()>,
        target: TypeId,
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
            fact = fact.join(self.read(
                owner,
                Key::Stored(
                    frame.clone(),
                    crate::value_access::ValueSourceSelection {
                        boundary,
                        slot: crate::value_access::ValueSourceSlot::Local(local),
                        path: path.clone(),
                    },
                    target,
                ),
            )?);
        }
        Ok(fact)
    }
    pub(in super::super::super) fn source_stored_data(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        selected: &crate::value_access::ValueSourceSelection,
        target: TypeId,
    ) -> Result {
        use crate::value_access::ValueSourceRead;
        let Some(source) = self
            .evidence
            .value_sources(frame.code())
            .and_then(|graph| graph.flow())
            .and_then(|flow| flow.read(selected.boundary, selected.slot))
        else {
            return Ok(Fact::Unavailable);
        };
        match source {
            ValueSourceRead::Empty => Ok(Fact::Empty),
            ValueSourceRead::Value(value) => self.read(
                owner,
                Key::Data(frame.clone(), value, selected.path.clone(), target),
            ),
            ValueSourceRead::Incoming(previous) => {
                let mut fact = Fact::Empty;
                for boundary in previous {
                    let next = crate::value_access::ValueSourceSelection {
                        boundary,
                        ..selected.clone()
                    };
                    fact = fact.join(self.read(owner, Key::Stored(frame.clone(), next, target))?);
                }
                Ok(fact)
            }
        }
    }
    pub(in super::super::super) fn source_data_result(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        path: &ValuePath<()>,
        target: TypeId,
    ) -> Result {
        let Some(graph) = self.evidence.value_sources(frame.code()) else {
            return Ok(Fact::Unavailable);
        };
        let returned: Vec<_> = graph.returned().iter().copied().collect();
        let mut fact = Fact::Empty;
        for value in returned {
            let Some(original) = self
                .source_node(frame, value, path)
                .and_then(|node| node.data)
                .and_then(|data| {
                    self.program
                        .project_data_by(data, path, |(), length| length > 0)
                })
            else {
                return Ok(Fact::Unavailable);
            };
            let supplied = self.read(
                owner,
                Key::Data(frame.clone(), value, path.clone(), original),
            )?;
            let selected = match supplied {
                Fact::Data(key) => {
                    let projected = key.reproject(self.program, target).or_else(|error| {
                        if let nocter_model::ValueCode::Declaration(code) = frame.code() {
                            key.reproject_result(self.program, code, path, target)
                        } else {
                            Err(error)
                        }
                    });
                    projected.map_or(Fact::Unavailable, Fact::Data)
                }
                other => other,
            };
            fact = fact.join(selected);
        }
        Ok(fact)
    }
    pub(super) fn source_data_transport(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        value: ValueSourceId,
        kind: &ValueSource,
        path: &ValuePath<()>,
        target: TypeId,
    ) -> Result {
        match kind {
            ValueSource::CallResult(site) => {
                match self.read(owner, Key::Call(frame.clone(), *site))? {
                    Fact::Frame(callee) => {
                        self.read(owner, Key::DataResult(callee, path.clone(), target))
                    }
                    Fact::Empty => Ok(Fact::Empty),
                    _ => Ok(Fact::Unavailable),
                }
            }
            ValueSource::Index { source, .. } => self.read(
                owner,
                Key::Data(
                    frame.clone(),
                    *source,
                    ValuePath::new([ValueProjection::Array(())]).concat(path),
                    target,
                ),
            ),
            ValueSource::Replace {
                source,
                value: supplied,
                path: written,
                weak,
            } => {
                let tail = match replacement_path(written, path) {
                    ReplacementPath::Unchanged => {
                        return self.read(
                            owner,
                            Key::Data(frame.clone(), *source, path.clone(), target),
                        );
                    }
                    ReplacementPath::Supplied(tail) => tail,
                    ReplacementPath::Components => {
                        return self.source_data_components(owner, frame, value, path, target);
                    }
                };
                let fact = self.read(owner, Key::Data(frame.clone(), *supplied, tail, target))?;
                if *weak {
                    Ok(fact.join(self.read(
                        owner,
                        Key::Data(frame.clone(), *source, path.clone(), target),
                    )?))
                } else {
                    Ok(fact)
                }
            }
            ValueSource::CallWrite {
                source,
                path: written,
                ..
            } => match replacement_path(written, path) {
                ReplacementPath::Unchanged => self.read(
                    owner,
                    Key::Data(frame.clone(), *source, path.clone(), target),
                ),
                ReplacementPath::Components => {
                    self.source_data_components(owner, frame, value, path, target)
                }
                ReplacementPath::Supplied(_) => {
                    self.source_call_write_data(owner, frame, kind, path, target)
                }
            },
            // Optional state and externally dependent whole-input writes remain unqualified.
            _ => Ok(Fact::Unavailable),
        }
    }
    pub(in super::super) fn source_data_components(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        value: ValueSourceId,
        path: &ValuePath<()>,
        target: TypeId,
    ) -> Result {
        let fields: Vec<_> = match self.program.data_schema(target) {
            Some(PayloadSchema::Aggregate(schema)) => schema
                .components()
                .map(|component| (component.field, component.data))
                .collect(),
            Some(PayloadSchema::Array(array)) if array.is_empty() => vec![],
            Some(PayloadSchema::Array(array)) => vec![(ValueField::Array(0), array.element())],
            _ => return Ok(Fact::Unavailable),
        };
        let mut values = Vec::new();
        let mut pending = false;
        for (field, data) in fields {
            let selected = path.concat(&crate::value_access::paths::known_by(
                &nocter_model::FieldPath::new([field]),
                |_| (),
            ));
            match self.read(owner, Key::Data(frame.clone(), value, selected, data))? {
                Fact::Data(data) => values.push((field, data)),
                Fact::Empty => pending = true,
                _ => return Ok(Fact::Unavailable),
            }
        }
        if pending {
            return Ok(Fact::Empty);
        }
        Ok(self
            .program
            .instantiate_components(target, values)
            .map_or(Fact::Unavailable, Fact::Data))
    }
}
