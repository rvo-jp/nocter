//! Close declaration-validated contextual schemas from exact source paths, not outer bindings.
use super::{
    Fact, Key, LocalOrigin, PayloadSchema, Query, Result, TypeId, ValuePath, ValueSource,
    ValueSourceId,
};
use nocter_declarations::value_access::CodeInstanceKey;
use nocter_model::{ValueField, ValueProjection};

impl Query<'_, '_> {
    pub(in super::super) fn source_data(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        value: ValueSourceId,
        path: &ValuePath<()>,
        target: TypeId,
    ) -> Result {
        let Some(node) = self.source_node(frame, value, path) else {
            return Ok(Fact::Unavailable);
        };
        if node.value == ValueSource::Empty {
            return Ok(Fact::Empty);
        }
        let Some(schema) = self.program.data_schema(target) else {
            return Ok(Fact::Unavailable);
        };
        if matches!(schema, PayloadSchema::Function(_)) {
            return Ok(
                match self.read(owner, Key::Value(frame.clone(), value, path.clone()))? {
                    Fact::Witness(actual) => self
                        .program
                        .instantiate_function(target, &actual)
                        .map_or(Fact::Unavailable, Fact::Data),
                    Fact::Empty => Fact::Empty,
                    _ => Fact::Unavailable,
                },
            );
        }
        if let ValueSource::Merge { ref inputs, .. } | ValueSource::Alternatives(ref inputs) =
            node.value
        {
            let mut fact = Fact::Empty;
            for &input in inputs {
                fact = fact
                    .join(self.read(owner, Key::Data(frame.clone(), input, path.clone(), target))?);
            }
            return Ok(fact);
        }
        // Representation only: no seed execution, positive payload or numeric proof is supplied.
        let source_data = node.data.and_then(|data| {
            self.program
                .project_data_by(data, path, |(), length| length > 0)
        });
        if source_data == Some(target)
            && let Ok(data) = self.program.instantiate_closed_data(target)
        {
            return Ok(Fact::Data(data));
        }
        match node.value {
            ValueSource::Input {
                origin: LocalOrigin::Capture,
                ..
            } => Ok(self
                .program
                .instantiate_environment(frame)
                .ok()
                .flatten()
                .and_then(|key| {
                    key.project_path(self.program, &super::concrete_path(path))
                        .ok()
                })
                .filter(|key| key.data() == target)
                .map_or(Fact::Unavailable, Fact::Data)),
            ValueSource::Adapt { source } => self.read(
                owner,
                Key::Data(frame.clone(), source, path.clone(), target),
            ),
            ValueSource::Select {
                source,
                path: prefix,
            } => self.read(
                owner,
                Key::Data(frame.clone(), source, prefix.concat(path), target),
            ),
            ValueSource::Aggregate(fields) if !path.is_root() => {
                let (selected, tail) = path
                    .fields()
                    .split_first()
                    .ok_or(super::InstantiationFailure::Unresolved)?;
                let mut fact = Fact::Empty;
                for (field, source) in fields {
                    if super::same_field(field, *selected) {
                        fact = fact.join(self.read(
                            owner,
                            Key::Data(
                                frame.clone(),
                                source,
                                ValuePath::new(tail.iter().copied()),
                                target,
                            ),
                        )?);
                    }
                }
                Ok(fact)
            }
            ValueSource::Aggregate(fields) => self.source_aggregate(owner, frame, &fields, target),
            ValueSource::Repeat { seed } => {
                self.source_repetition(owner, frame, seed, path, target)
            }
            // General carrier returns, optional state and contextual writes remain unqualified.
            _ => Ok(Fact::Unavailable),
        }
    }
    fn source_aggregate(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        fields: &[(ValueField, ValueSourceId)],
        target: TypeId,
    ) -> Result {
        let array = match self.program.data_schema(target) {
            Some(PayloadSchema::Array(array)) => Some(array),
            _ => None,
        };
        if let Some(array) = array
            && u64::try_from(fields.len()) != Ok(array.length())
        {
            return Ok(Fact::Unavailable);
        }
        let mut values = Vec::new();
        let mut element = Fact::Empty;
        let mut pending = false;
        for &(field, source) in fields {
            let Some(component) = self.program.component(target, field) else {
                return Ok(Fact::Unavailable);
            };
            match self.read(
                owner,
                Key::Data(frame.clone(), source, ValuePath::default(), component.data),
            )? {
                Fact::Data(data) if array.is_some() => {
                    element = element.join(Fact::Data(data));
                }
                Fact::Data(data) => values.push((field, data)),
                Fact::Empty => pending = true,
                _ => return Ok(Fact::Unavailable),
            }
        }
        if pending {
            return Ok(Fact::Empty);
        }
        if let Some(array) = array
            && !array.is_empty()
        {
            let Fact::Data(data) = element else {
                return Ok(Fact::Unavailable);
            };
            values.push((ValueField::Array(0), data));
        }
        Ok(self
            .program
            .instantiate_components(target, values)
            .map_or(Fact::Unavailable, Fact::Data))
    }
    fn source_repetition(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        seed: ValueSourceId,
        path: &ValuePath<()>,
        target: TypeId,
    ) -> Result {
        if !path.is_root() {
            let Some((ValueProjection::Array(()), tail)) = path.fields().split_first() else {
                return Ok(Fact::Unavailable);
            };
            return self.read(
                owner,
                Key::Data(
                    frame.clone(),
                    seed,
                    ValuePath::new(tail.iter().copied()),
                    target,
                ),
            );
        }
        let Some(PayloadSchema::Array(array)) = self.program.data_schema(target) else {
            return Ok(Fact::Unavailable);
        };
        if array.is_empty() {
            return Ok(self
                .program
                .instantiate_closed_data(target)
                .map_or(Fact::Unavailable, Fact::Data));
        }
        Ok(
            match self.read(
                owner,
                Key::Data(frame.clone(), seed, ValuePath::default(), array.element()),
            )? {
                Fact::Data(data) => self
                    .program
                    .instantiate_components(target, [(ValueField::Array(0), data)])
                    .map_or(Fact::Unavailable, Fact::Data),
                Fact::Empty => Fact::Empty,
                _ => Fact::Unavailable,
            },
        )
    }
}
