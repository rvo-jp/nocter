//! Source-local names and slots over one declaration writer. No type/body admission occurs here.
use super::{
    CodeSource, Failure, ModuleSourcesFailure, PairedModuleSources,
    headers::{self, Header, RecordHeader},
    projection,
};
use crate::{SurfaceDeclarationId, SurfaceDeclarationKind, SurfaceSourceId};
use nocter_declarations::value_access::{
    DeclarationId, HeaderPublication, Program, Reservations, ScopeId,
};
use nocter_model::{AccessCodeId, AccessFieldId, NominalTypeId, Symbol};
use nocter_syntax::{BoundSyntax, SyntaxToken};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Closed names and reserved identities only. No normalized contract, payload, Copy or body proof.
#[derive(Debug)]
pub struct ScopedModuleSources<'syntax> {
    paired: PairedModuleSources<'syntax>,
    scopes: Box<[ScopeId]>,
    declarations: Box<[Option<DeclarationId>]>,
    fields: Box<[Option<AccessFieldId>]>,
    headers: HeaderPublication,
}
#[derive(Debug)]
enum ParsedHeader {
    Code(Header),
    Record(RecordHeader),
}
impl<'syntax> PairedModuleSources<'syntax> {
    /// Reserve one semantic identity per paired declaration and close direct authored lookup.
    /// # Errors
    /// Requires import binding evidence before processing `use`; rejects name conflicts locally.
    pub fn reserve_names(self) -> Result<ScopedModuleSources<'syntax>, ModuleSourcesFailure> {
        let inventory = &self.inventory;
        if !inventory.imports().is_empty() {
            return Err(ModuleSourcesFailure::Access(
                Failure::ModuleImportBindingsRequired(
                    inventory
                        .imports()
                        .iter()
                        .map(crate::SurfaceImport::node)
                        .collect(),
                ),
            ));
        }
        let parsed = collect_headers(&self)?;
        let mut reservations = Reservations::with_symbols(inventory.surface.symbol_handle());
        let scopes: Box<[_]> = inventory
            .source_entries()
            .map(|(source, _)| {
                if source.index() == 0 {
                    ScopeId::ROOT
                } else {
                    reservations.new_scope()
                }
            })
            .collect();
        let mut declarations = vec![None; inventory.declarations().len()];
        reserve_canonical(
            &self,
            &parsed,
            &scopes,
            &mut reservations,
            &mut declarations,
        )?;
        for (id, _) in inventory.declaration_entries() {
            declarations[id.index()] = declarations[self.contracts.representative(id).index()];
        }
        bind_sources(&self, &scopes, &declarations, &mut reservations)?;
        let definitions = reservations
            .finish()
            .map_err(|violations| project_namespace(&self, &parsed, &declarations, violations))?;
        let fields = field_mappings(&self, &definitions, &declarations)?;
        Ok(ScopedModuleSources {
            paired: self,
            scopes,
            declarations: declarations.into(),
            fields,
            headers: definitions.into_publication(),
        })
    }
}
impl<'syntax> ScopedModuleSources<'syntax> {
    #[must_use]
    pub const fn paired(&self) -> &PairedModuleSources<'syntax> {
        &self.paired
    }
    #[must_use]
    pub fn scope(&self, source: SurfaceSourceId) -> Option<ScopeId> {
        self.scopes.get(source.index()).copied()
    }
    #[must_use]
    pub fn declaration(&self, source: SurfaceDeclarationId) -> Option<DeclarationId> {
        self.declarations.get(source.index()).copied().flatten()
    }
    #[must_use]
    pub fn field(&self, source: SurfaceDeclarationId) -> Option<AccessFieldId> {
        self.fields.get(source.index()).copied().flatten()
    }
    #[must_use]
    pub fn program(&self) -> Program {
        self.headers.snapshot()
    }
    #[must_use]
    pub fn parameter_ids(&self, code: AccessCodeId) -> Option<&[nocter_model::AccessParameterId]> {
        self.headers.parameter_ids(code)
    }
    #[must_use]
    pub fn field_ids(&self, record: NominalTypeId) -> Option<&[AccessFieldId]> {
        self.headers.field_ids(record)
    }
}
fn project_namespace(
    paired: &PairedModuleSources<'_>,
    parsed: &BTreeMap<SurfaceDeclarationId, ParsedHeader>,
    declarations: &[Option<DeclarationId>],
    violations: Vec<nocter_declarations::value_access::DeclarationViolation>,
) -> ModuleSourcesFailure {
    let mut codes = BTreeMap::new();
    let mut records = BTreeMap::new();
    for (id, header) in parsed {
        if paired.contracts.representative(*id) != *id {
            continue;
        }
        match (declarations[id.index()], header) {
            (Some(DeclarationId::Code(id)), ParsedHeader::Code(header)) => {
                codes.insert(id, CodeSource::from_header(header));
            }
            (Some(DeclarationId::Record(id)), ParsedHeader::Record(header)) => {
                records.insert(id, Arc::clone(&header.source));
            }
            _ => {}
        }
    }
    let trees = paired
        .inventory
        .sources()
        .iter()
        .map(crate::SurfaceSource::syntax)
        .collect::<Vec<_>>();
    ModuleSourcesFailure::Access(projection::project_violations_in(
        &trees, &codes, &records, violations,
    ))
}
fn field_mappings(
    paired: &PairedModuleSources<'_>,
    definitions: &nocter_declarations::value_access::Builder,
    declarations: &[Option<DeclarationId>],
) -> Result<Box<[Option<AccessFieldId>]>, ModuleSourcesFailure> {
    let inventory = &paired.inventory;
    let mut fields = vec![None; inventory.declarations().len()];
    for (contract, declaration) in inventory.declaration_entries() {
        let Some(DeclarationId::Record(record)) = declarations[contract.index()] else {
            continue;
        };
        if paired.contracts.representative(contract) != contract
            || declaration.kind() != SurfaceDeclarationKind::Struct
        {
            continue;
        }
        let representation = paired
            .contracts
            .representation(contract)
            .unwrap_or(contract);
        let slots = definitions.field_ids(record).ok_or_else(integrity)?;
        let authored = inventory
            .declaration_entries()
            .filter(|(_, field)| {
                field.owner() == Some(representation)
                    && field.kind() == SurfaceDeclarationKind::Field
            })
            .collect::<Vec<_>>();
        if authored.len() != slots.len() {
            return Err(integrity());
        }
        for ((field, _), slot) in authored.into_iter().zip(slots) {
            fields[paired.contracts.representative(field).index()] = Some(*slot);
        }
    }
    for (id, _) in inventory.declaration_entries() {
        fields[id.index()] = fields[paired.contracts.representative(id).index()];
    }
    Ok(fields.into())
}
fn integrity() -> ModuleSourcesFailure {
    ModuleSourcesFailure::Access(Failure::InconsistentSyntax)
}
fn syntax<'a>(
    paired: &'a PairedModuleSources<'_>,
    source: SurfaceSourceId,
) -> Result<BoundSyntax<'a>, ModuleSourcesFailure> {
    let inventory = &paired.inventory;
    let tree = inventory
        .sources()
        .get(source.index())
        .ok_or_else(integrity)?
        .syntax();
    BoundSyntax::new(
        inventory
            .source_map()
            .get(tree.source())
            .ok_or_else(integrity)?,
        tree,
    )
    .ok_or_else(integrity)
}
fn symbol(
    paired: &PairedModuleSources<'_>,
    source: SurfaceSourceId,
    token: SyntaxToken,
) -> Result<Symbol, ModuleSourcesFailure> {
    let syntax = syntax(paired, source)?;
    paired
        .inventory
        .surface
        .symbols()
        .get(syntax.text_at(token.range()).ok_or_else(integrity)?)
        .ok_or_else(integrity)
}
fn collect_headers(
    paired: &PairedModuleSources<'_>,
) -> Result<BTreeMap<SurfaceDeclarationId, ParsedHeader>, ModuleSourcesFailure> {
    paired
        .inventory
        .declaration_entries()
        .filter_map(|(id, declaration)| {
            let header = match declaration.kind() {
                SurfaceDeclarationKind::Function | SurfaceDeclarationKind::PrimitiveFunction => {
                    syntax(paired, declaration.source()).and_then(|s| {
                        headers::function(s, declaration.node())
                            .map(ParsedHeader::Code)
                            .map_err(ModuleSourcesFailure::Access)
                    })
                }
                SurfaceDeclarationKind::Constant | SurfaceDeclarationKind::Static => {
                    syntax(paired, declaration.source()).and_then(|s| {
                        headers::initializer(s, declaration.node())
                            .map(ParsedHeader::Code)
                            .map_err(ModuleSourcesFailure::Access)
                    })
                }
                SurfaceDeclarationKind::Struct => {
                    syntax(paired, declaration.source()).and_then(|s| {
                        headers::record(s, declaration.node())
                            .map(ParsedHeader::Record)
                            .map_err(ModuleSourcesFailure::Access)
                    })
                }
                SurfaceDeclarationKind::Field => return None,
                _ => return Some(Err(integrity())),
            };
            Some(header.map(|header| (id, header)))
        })
        .collect()
}
fn reserve_canonical(
    paired: &PairedModuleSources<'_>,
    parsed: &BTreeMap<SurfaceDeclarationId, ParsedHeader>,
    scopes: &[ScopeId],
    reservations: &mut Reservations,
    declarations: &mut [Option<DeclarationId>],
) -> Result<(), ModuleSourcesFailure> {
    let mut ordered = Vec::new();
    for (id, header) in parsed {
        if paired.contracts.representative(*id) != *id {
            continue;
        }
        let declaration = paired.inventory.declarations()[id.index()];
        let (record, name) = match header {
            ParsedHeader::Code(header) => (false, header.name),
            ParsedHeader::Record(header) => (true, header.source.name),
        };
        ordered.push((
            !record,
            declaration.source(),
            symbol(paired, declaration.source(), name)?,
            *id,
        ));
    }
    ordered.sort_unstable();
    for (_, _, name, id) in ordered {
        let declaration = paired.inventory.declarations()[id.index()];
        let semantic = match parsed.get(&id).ok_or_else(integrity)? {
            ParsedHeader::Code(header) => reservations
                .reserve_in(
                    scopes[declaration.source().index()],
                    name,
                    header.parameters.len(),
                )
                .map(DeclarationId::Code),
            ParsedHeader::Record(_) => {
                let definition = paired.contracts.representation(id).unwrap_or(id);
                let physical = paired.inventory.declarations()[definition.index()];
                let Some(ParsedHeader::Record(header)) = parsed.get(&definition) else {
                    return Err(integrity());
                };
                reservations
                    .reserve_record_in(scopes[physical.source().index()], name, header.fields.len())
                    .map(DeclarationId::Record)
            }
        }
        .map_err(|_| integrity())?;
        declarations[id.index()] = Some(semantic);
    }
    Ok(())
}
fn bind_sources(
    paired: &PairedModuleSources<'_>,
    scopes: &[ScopeId],
    declarations: &[Option<DeclarationId>],
    reservations: &mut Reservations,
) -> Result<(), ModuleSourcesFailure> {
    let mut authored = BTreeMap::<SurfaceSourceId, Vec<(Symbol, DeclarationId)>>::new();
    for (id, declaration) in paired.inventory.declaration_entries() {
        if let Some(semantic) = declarations[id.index()] {
            authored.entry(declaration.source()).or_default().push((
                symbol(
                    paired,
                    declaration.source(),
                    declaration.name().ok_or_else(integrity)?,
                )?,
                semantic,
            ));
        }
    }
    for (source, _) in paired.inventory.source_entries() {
        let visible: BTreeSet<_> = std::iter::once(source)
            .chain(
                paired
                    .inventory
                    .source_visibilities()
                    .iter()
                    .filter(|edge| edge.source() == source)
                    .map(|edge| edge.target()),
            )
            .collect();
        for author in visible {
            for &(name, declaration) in authored.get(&author).map_or(&[][..], Vec::as_slice) {
                reservations
                    .bind(scopes[source.index()], name, declaration)
                    .map_err(|_| integrity())?;
            }
        }
    }
    Ok(())
}
