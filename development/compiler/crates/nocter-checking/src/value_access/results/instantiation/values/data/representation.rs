//! Inactive outcome payloads inherit representation constraints from original value consumers.
//! This query supplies metadata only. Positive witnesses still require original producing paths.
use super::{
    CodeInstanceKey, Fact, Key, PayloadSchema, Query, Result, TypeId, ValuePath, ValueSource,
    ValueSourceId,
};

use nocter_declarations::value_access::DataInstanceKey;
use nocter_model::ValueField;

impl Query<'_, '_> {
    pub(super) fn outcome_metadata(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        value: ValueSourceId,
        target: TypeId,
        values: &mut Vec<(ValueField, DataInstanceKey)>,
    ) -> std::result::Result<Option<Fact>, super::super::InstantiationFailure> {
        if let Some(PayloadSchema::Optional(primary) | PayloadSchema::Fallible(primary)) =
            self.program.data_schema(target)
        {
            let mut expected = vec![(ValueField::Variant(true), primary)];
            if matches!(
                self.program.data_schema(target),
                Some(PayloadSchema::Fallible(_))
            ) {
                expected.push((
                    ValueField::Variant(false),
                    self.program
                        .data_type(nocter_model::BuiltinType::Error)
                        .ok_or(super::super::InstantiationFailure::Unresolved)?,
                ));
            }
            for (field, data) in expected {
                if values.iter().any(|(selected, _)| *selected == field) {
                    continue;
                }
                if let Ok(key) = self.program.instantiate_closed_data(data) {
                    values.push((field, key));
                    continue;
                }
                let fact = self.read(
                    owner,
                    Key::Representation(frame.clone(), value, ValuePath::default(), target),
                )?;
                let Fact::Data(key) = &fact else {
                    return Ok(Some(fact));
                };
                for (field, actual) in values.iter() {
                    let Ok(expected) = key.component(self.program, *field) else {
                        return Ok(Some(Fact::Unavailable));
                    };
                    if !actual.has_same_representation(self.program, &expected) {
                        return Ok(Some(Fact::Unavailable));
                    }
                }
                return Ok(Some(fact));
            }
        }
        Ok(None)
    }

    pub(in super::super::super) fn source_representation(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        value: ValueSourceId,
        path: &ValuePath<()>,
        target: TypeId,
    ) -> Result {
        if self.source_node(frame, value, path).is_none() {
            return Ok(Fact::Unavailable);
        }
        let Some(graph) = self.evidence.value_sources(frame.code()) else {
            return Ok(Fact::Unavailable);
        };
        let mut consumers = Vec::new();
        for (id, node) in graph.nodes() {
            self.work
                .step()
                .map_err(|_| super::super::InstantiationFailure::WorkLimit)?;
            let forwards = match &node.value {
                ValueSource::Adapt { source } => *source == value,
                ValueSource::Merge { inputs, .. } | ValueSource::Alternatives(inputs) => {
                    inputs.contains(&value)
                }
                _ => false,
            };
            if forwards {
                consumers.push((id, path.clone()));
            }
            if let ValueSource::Aggregate(fields) = &node.value {
                for (field, source) in fields {
                    self.work
                        .step()
                        .map_err(|_| super::super::InstantiationFailure::WorkLimit)?;
                    if *source == value {
                        let prefix = crate::value_access::paths::known_by(
                            &nocter_model::FieldPath::new([*field]),
                            |_| (),
                        );
                        consumers.push((id, prefix.concat(path)));
                    }
                }
            }
        }
        let returned = graph.returned().contains(&value);
        let mut fact = Fact::Empty;
        for (consumer, selected) in consumers {
            fact = fact.join(self.read(
                owner,
                Key::Data(frame.clone(), consumer, selected.clone(), target),
            )?);
            fact = fact.join(self.read(
                owner,
                Key::Representation(frame.clone(), consumer, selected, target),
            )?);
        }
        if returned {
            fact =
                fact.join(self.read(owner, Key::DataResult(frame.clone(), path.clone(), target))?);
        }
        Ok(fact)
    }
}
