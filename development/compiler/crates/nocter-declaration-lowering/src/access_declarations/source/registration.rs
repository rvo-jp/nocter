//! Reserve exact executable roots before closing the declaration name domain.
use super::{CodeHeaderSource, CodeSource, Sources};
use crate::access_declarations::{
    Failure,
    headers::{Header, RecordHeader},
};
use nocter_declarations::value_access::Reservations;
use nocter_model::{AccessCodeId, NominalTypeId};
use nocter_syntax::NodeId;
use std::{collections::BTreeMap, sync::Arc};

impl Sources {
    pub(in crate::access_declarations) fn reserve(
        reservations: &mut Reservations,
        function_headers: &BTreeMap<AccessCodeId, Header>,
        record_headers: &BTreeMap<NominalTypeId, RecordHeader>,
    ) -> Result<Self, Failure> {
        let mut sources: BTreeMap<_, _> = function_headers
            .iter()
            .map(|(&id, h)| {
                (
                    id,
                    CodeSource {
                        header: Some(CodeHeaderSource {
                            name: h.name,
                            parameters: h.parameters.iter().map(|p| p.name).collect(),
                            parameter_types: h.parameters.iter().map(|p| p.ty).collect(),
                            result: h.result_syntax,
                        }),
                        body: h.body,
                    },
                )
            })
            .collect();
        let mut register = |declaration, position, lengths: Vec<NodeId>| -> Result<(), Failure> {
            for body in lengths {
                let id = reservations
                    .reserve_type_expression(
                        nocter_declarations::value_access::TypeExpressionSource {
                            declaration,
                            position,
                        },
                    )
                    .map_err(|_| Failure::InconsistentSyntax)?;
                sources.insert(id, CodeSource { header: None, body });
            }
            Ok(())
        };
        for (&id, header) in record_headers {
            for (index, field) in header.fields.iter().enumerate() {
                let mut lengths = Vec::new();
                field.length_expressions(&mut lengths);
                register(
                    id.into(),
                    nocter_declarations::value_access::HeaderPosition::Field(index),
                    lengths,
                )?;
            }
        }
        for (&id, header) in function_headers {
            for (index, parameter) in header.parameters.iter().enumerate() {
                let mut lengths = Vec::new();
                parameter.data.length_expressions(&mut lengths);
                register(
                    id.into(),
                    nocter_declarations::value_access::HeaderPosition::Parameter(index),
                    lengths,
                )?;
            }
            let mut lengths = Vec::new();
            header.result_data.length_expressions(&mut lengths);
            register(
                id.into(),
                nocter_declarations::value_access::HeaderPosition::Result,
                lengths,
            )?;
        }
        let record_sources: BTreeMap<_, _> = record_headers
            .iter()
            .map(|(&id, header)| (id, header.source.clone()))
            .collect();
        Ok(Self {
            codes: Arc::new(sources),
            records: Arc::new(record_sources),
        })
    }
}
