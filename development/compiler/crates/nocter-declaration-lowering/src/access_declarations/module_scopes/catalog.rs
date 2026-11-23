//! Physical header occurrences and executable roots over the already chosen semantic identities.
use super::{FieldPosition, ModuleSourcesFailure, PairedModuleSources, ParsedHeader, integrity};
use crate::access_declarations::{CodeSource, source::Sources};
use crate::{SurfaceDeclarationId, SurfaceSourceId};
use nocter_declarations::value_access::{
    DeclarationId, HeaderPosition, Reservations, ScopeId, TypeExpressionSource,
};
use nocter_model::AccessCodeId;
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) struct Catalog {
    pub parsed: BTreeMap<SurfaceDeclarationId, ParsedHeader>,
    pub sources: Sources,
    pub owners: BTreeMap<AccessCodeId, CodeOwner>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct CodeOwner {
    pub source: SurfaceSourceId,
    pub lookup: ScopeId,
}
impl Catalog {
    pub fn reserve(
        paired: &PairedModuleSources<'_>,
        parsed: BTreeMap<SurfaceDeclarationId, ParsedHeader>,
        declaration_scopes: &[ScopeId],
        declarations: &[Option<DeclarationId>],
        fields: &[Option<FieldPosition>],
        reservations: &mut Reservations,
    ) -> Result<Self, ModuleSourcesFailure> {
        let mut codes = BTreeMap::new();
        let mut records = BTreeMap::new();
        let mut owners = BTreeMap::new();
        // Complete definitions provide executable roots. Primitive declarations retain a header
        // source and physical scope, but never acquire a fabricated body.
        for (raw, header) in &parsed {
            let physical = paired.inventory.declarations()[raw.index()];
            let owner = CodeOwner {
                source: physical.source(),
                lookup: declaration_scopes[raw.index()],
            };
            let semantic = declarations[raw.index()].ok_or_else(integrity)?;
            match (semantic, header) {
                (DeclarationId::Code(code), header) => {
                    if let Some(source) = header.executable_source()
                        && (codes.insert(code, source).is_some()
                            || owners.insert(code, owner).is_some())
                    {
                        return Err(integrity());
                    }
                }
                (DeclarationId::Record(record), ParsedHeader::Record(header)) => {
                    let contract = paired.contracts.representative(*raw);
                    if paired
                        .contracts
                        .representation(contract)
                        .unwrap_or(contract)
                        == *raw
                        && records.insert(record, header.source.clone()).is_some()
                    {
                        return Err(integrity());
                    }
                }
                _ => {}
            }
        }
        for (raw, header) in &parsed {
            let physical = paired.inventory.declarations()[raw.index()];
            let owner = CodeOwner {
                source: physical.source(),
                lookup: declaration_scopes[raw.index()],
            };
            let semantic = declarations[raw.index()].ok_or_else(integrity)?;
            let mut register = |position,
                                lengths: Vec<nocter_syntax::NodeId>|
             -> Result<(), ModuleSourcesFailure> {
                for body in lengths {
                    let id = reservations
                        .reserve_type_expression_in(
                            owner.lookup,
                            TypeExpressionSource {
                                declaration: semantic,
                                position,
                            },
                        )
                        .map_err(|_| integrity())?;
                    codes.insert(id, CodeSource::expression(body));
                    owners.insert(id, owner);
                }
                Ok(())
            };
            match header {
                ParsedHeader::Test { .. } => {}
                ParsedHeader::Code(header) => {
                    for (index, parameter) in header.parameters.iter().enumerate() {
                        let mut lengths = Vec::new();
                        parameter.data.length_expressions(&mut lengths);
                        register(HeaderPosition::Parameter(index), lengths)?;
                    }
                    let mut lengths = Vec::new();
                    header.result_data.length_expressions(&mut lengths);
                    register(HeaderPosition::Result, lengths)?;
                }
                ParsedHeader::Record(header) => {
                    let authored = paired
                        .inventory
                        .surface
                        .record_fields(*raw)
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>();
                    if authored.len() != header.fields.len() {
                        return Err(integrity());
                    }
                    for (data, raw_field) in header.fields.iter().zip(authored) {
                        let position = fields[raw_field.index()].ok_or_else(integrity)?.index;
                        let mut lengths = Vec::new();
                        data.length_expressions(&mut lengths);
                        register(HeaderPosition::Field(position), lengths)?;
                    }
                }
            }
        }
        let sources = Sources::new(codes, records).map_err(ModuleSourcesFailure::Access)?;
        Ok(Self {
            parsed,
            sources,
            owners,
        })
    }
}
