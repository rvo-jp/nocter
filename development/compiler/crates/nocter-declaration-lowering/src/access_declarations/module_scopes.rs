//! Source-local names, physical headers and progressive metadata over one declaration writer.
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

mod catalog;
mod normalization;
mod record_views;

/// Closed source names and a progressive header writer. Reservation alone grants no typed metadata;
/// selected header publication grants no checked-body, primitive-authority or accepted-Unit proof.
#[derive(Debug)]
pub struct ScopedModuleSources<'syntax> {
    headers: HeaderPublication,
    binding: Arc<ModuleSourceBinding<'syntax>>,
}
/// Closed header metadata and its exact current physical sources; no cross-revision reuse proof.
#[derive(Debug)]
pub struct ModuleContext<'syntax> {
    program: Arc<Program>,
    binding: Arc<ModuleSourceBinding<'syntax>>,
}
impl ModuleContext<'_> {
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }
    /// Resolve an exported name without widening its authored audience.
    #[must_use]
    pub fn lookup_export(
        &self,
        module: &crate::ModuleIdentity,
        caller: &crate::ModuleIdentity,
        namespace: nocter_declarations::value_access::Namespace,
        name: Symbol,
    ) -> Option<DeclarationId> {
        self.binding.exports.lookup(
            &self.binding.paired,
            &self.program,
            module,
            caller,
            namespace,
            name,
        )
    }
    /// Open the closed metadata in this exact source binding.
    /// # Errors
    /// Rejects inconsistent physical source metadata; grants no body acceptance.
    pub fn current(&self) -> Result<super::Context<'_>, ModuleSourcesFailure> {
        self.binding.base_view(Arc::clone(&self.program))
    }
}
/// One immutable physical-source/semantic-slot pairing, retained independently of header writes.
/// No caller can construct it from separately supplied syntax, scopes or declaration identities.
#[derive(Debug)]
pub struct ModuleSourceBinding<'syntax> {
    paired: PairedModuleSources<'syntax>,
    scopes: Box<[ScopeId]>,
    declarations: Box<[Option<DeclarationId>]>,
    fields: Box<[Option<AccessFieldId>]>,
    catalog: catalog::Catalog,
    exports: super::module_imports::ModuleExports,
    lexical: super::module_imports::LexicalScopes,
}
#[derive(Debug)]
enum ParsedHeader {
    Code(Header),
    Record(RecordHeader),
}
#[derive(Clone, Copy)]
struct FieldPosition {
    record: NominalTypeId,
    index: usize,
}
impl<'syntax> PairedModuleSources<'syntax> {
    /// Reserve one semantic identity per paired declaration and close direct authored lookup.
    /// # Errors
    /// Rejects invalid imports and authored source-local name conflicts.
    pub fn reserve_names(self) -> Result<ScopedModuleSources<'syntax>, ModuleSourcesFailure> {
        let inventory = &self.inventory;
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
        for (source, physical) in inventory.source_entries() {
            reservations
                .set_scope_module(scopes[source.index()], physical.module().clone())
                .map_err(|_| integrity())?;
        }
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
        let (exports, lexical) = super::module_imports::ModuleExports::bind(
            &self,
            &scopes,
            &declarations,
            &mut reservations,
        )?;
        let field_positions = field_positions(&self, &declarations);
        let catalog = catalog::Catalog::reserve(
            &self,
            parsed,
            &scopes,
            &declarations,
            &field_positions,
            &mut reservations,
        )?;
        let definitions = reservations.finish().map_err(|violations| {
            project_namespace(&self, &catalog.parsed, &declarations, violations)
        })?;
        let fields = field_positions
            .iter()
            .map(|position| {
                position
                    .map(|position| {
                        definitions
                            .field_ids(position.record)
                            .and_then(|fields| fields.get(position.index))
                            .copied()
                            .ok_or_else(integrity)
                    })
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        Ok(ScopedModuleSources {
            headers: definitions.into_publication(),
            binding: Arc::new(ModuleSourceBinding {
                paired: self,
                scopes,
                declarations: declarations.into(),
                fields,
                catalog,
                exports,
                lexical,
            }),
        })
    }
}
impl<'syntax> ScopedModuleSources<'syntax> {
    #[must_use]
    pub fn paired(&self) -> &PairedModuleSources<'syntax> {
        &self.binding.paired
    }
    #[must_use]
    pub fn scope(&self, source: SurfaceSourceId) -> Option<ScopeId> {
        self.binding.scopes.get(source.index()).copied()
    }
    #[must_use]
    pub fn declaration(&self, source: SurfaceDeclarationId) -> Option<DeclarationId> {
        self.binding
            .declarations
            .get(source.index())
            .copied()
            .flatten()
    }
    #[must_use]
    pub fn field(&self, source: SurfaceDeclarationId) -> Option<AccessFieldId> {
        self.binding.fields.get(source.index()).copied().flatten()
    }
    #[must_use]
    pub fn program(&self) -> Program {
        self.headers.snapshot()
    }
    /// Look up a promised module export under its effective audience; grants no header readiness.
    #[must_use]
    pub fn lookup_export(
        &self,
        module: &crate::ModuleIdentity,
        caller: &crate::ModuleIdentity,
        namespace: nocter_declarations::value_access::Namespace,
        name: Symbol,
    ) -> Option<DeclarationId> {
        self.binding.exports.lookup(
            &self.binding.paired,
            &self.headers.snapshot(),
            module,
            caller,
            namespace,
            name,
        )
    }
    /// Exact executable syntax for a definition or registered header count; no body acceptance.
    #[must_use]
    pub fn code_source(&self, code: AccessCodeId) -> Option<&CodeSource> {
        self.binding.catalog.sources.codes.get(&code)
    }
    /// The physical source whose scope applies to the executable root.
    #[must_use]
    pub fn source_for_code(&self, code: AccessCodeId) -> Option<SurfaceSourceId> {
        self.binding.catalog.owners.get(&code).copied()
    }
    /// Retain source pairing without borrowing the mutable header writer.
    #[must_use]
    pub fn source_binding(&self) -> Arc<ModuleSourceBinding<'syntax>> {
        Arc::clone(&self.binding)
    }
    /// All physical count suppliers, including both occurrences of a repeated header.
    pub fn header_expression_ids(&self) -> impl Iterator<Item = AccessCodeId> + '_ {
        self.binding.catalog.sources.expression_ids()
    }
    /// Check bodyless markers before ordinary module source orchestration.
    /// No primitive authority is inferred from an inventoried module or declaration name.
    /// # Errors
    /// Reports an unselected primitive at its actual authored declaration.
    pub fn validate_primitive_bindings(&self) -> Result<(), ModuleSourcesFailure> {
        let mut bindings = super::PrimitiveBindings::new(None);
        for (raw, header) in &self.binding.catalog.parsed {
            if let ParsedHeader::Code(header) = header {
                let Some(DeclarationId::Code(code)) = self.binding.declarations[raw.index()] else {
                    return Err(integrity());
                };
                let physical = self.binding.paired.inventory.declarations()[raw.index()];
                bindings
                    .select(
                        syntax(&self.binding.paired, physical.source())?,
                        None,
                        code,
                        header,
                    )
                    .map_err(ModuleSourcesFailure::Access)?;
            }
        }
        Ok(())
    }
    #[must_use]
    pub fn code_ids(&self) -> impl ExactSizeIterator<Item = AccessCodeId> + '_ {
        self.headers.code_ids()
    }
    pub fn declaration_ids(&self) -> impl Iterator<Item = DeclarationId> + '_ {
        self.headers.declaration_ids()
    }
    /// Classify a requested header cycle through the common declaration owner.
    #[must_use]
    pub fn reject_cycle(&self, path: &[DeclarationId]) -> ModuleSourcesFailure {
        self.project_header_failure(self.headers.reject_cycle(path), None)
    }
    /// Consume a fully advanced writer into one immutable current-source product.
    /// # Errors
    /// Requires every reserved named and anonymous header to be ready.
    pub fn seal(self) -> Result<ModuleContext<'syntax>, ModuleSourcesFailure> {
        if !self.program().headers_complete() {
            return Err(integrity());
        }
        let program = self.headers.finish().map_err(|_| integrity())?;
        self.binding
            .paired
            .inventory
            .audiences
            .verify_signatures(
                &self.binding.paired.inventory.surface,
                &self.binding.declarations,
                &self.binding.fields,
                &program,
            )
            .map_err(ModuleSourcesFailure::Access)?;
        Ok(ModuleContext {
            program: Arc::new(program),
            binding: self.binding,
        })
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
impl ModuleSourceBinding<'_> {
    /// Authored top-level root contracts only. This enumerates promises, not ready API evidence.
    pub fn exports(
        &self,
    ) -> impl Iterator<Item = (SurfaceDeclarationId, DeclarationId, &super::ModuleAudience)> + '_
    {
        self.paired
            .inventory
            .declaration_entries()
            .filter_map(|(raw, declaration)| {
                if declaration.owner().is_some() {
                    return None;
                }
                Some((
                    raw,
                    self.declarations[raw.index()]?,
                    self.paired.inventory.audience(raw)?,
                ))
            })
    }
    pub(super) fn lexical_scope(
        &self,
        source: nocter_source::SourceId,
        range: nocter_source::TextRange,
    ) -> Option<ScopeId> {
        self.lexical.at(source, range)
    }
    pub(super) fn block_import_names(
        &self,
        syntax: BoundSyntax<'_>,
        node: nocter_syntax::NodeId,
    ) -> &[SyntaxToken] {
        self.lexical.names(syntax, node)
    }
    pub(super) fn header_scopes(
        &self,
    ) -> Result<Vec<super::definitions::HeaderScope<'_>>, super::SourceMismatch> {
        self.paired
            .inventory
            .source_entries()
            .map(|(source, _)| {
                Ok(super::definitions::HeaderScope {
                    syntax: syntax(&self.paired, source)
                        .map_err(|_| super::SourceMismatch::InconsistentSyntax)?,
                    lookup: self.scopes[source.index()],
                })
            })
            .collect()
    }
    pub(super) fn code_scope(
        &self,
        code: AccessCodeId,
    ) -> Result<super::definitions::HeaderScope<'_>, super::SourceMismatch> {
        let source = self
            .catalog
            .owners
            .get(&code)
            .copied()
            .ok_or(super::SourceMismatch::UnknownSource)?;
        Ok(super::definitions::HeaderScope {
            syntax: syntax(&self.paired, source)
                .map_err(|_| super::SourceMismatch::InconsistentSyntax)?,
            lookup: self.scopes[source.index()],
        })
    }
    pub(super) fn syntax_for_origin(
        &self,
        origin: nocter_syntax::SyntaxOrigin,
    ) -> Result<BoundSyntax<'_>, super::SourceMismatch> {
        let source = match origin {
            nocter_syntax::SyntaxOrigin::Node(node) => node.source(),
            nocter_syntax::SyntaxOrigin::Token(token) => token.source(),
        };
        let physical = self
            .paired
            .inventory
            .source_entries()
            .find(|(_, entry)| entry.syntax().source() == source)
            .map(|(id, _)| id)
            .ok_or(super::SourceMismatch::UnknownSource)?;
        syntax(&self.paired, physical).map_err(|_| super::SourceMismatch::InconsistentSyntax)
    }
    /// Open the current snapshot before a particular body is selected.
    /// # Errors
    /// Rejects foreign writers and inconsistent physical source metadata.
    pub fn current<'binding>(
        &'binding self,
        headers: &ScopedModuleSources<'_>,
    ) -> Result<super::Context<'binding>, ModuleSourcesFailure> {
        if !std::ptr::eq(self, headers.binding.as_ref()) {
            return Err(ModuleSourcesFailure::Access(Failure::ChangedHeader));
        }
        self.base_view(Arc::new(headers.program()))
    }
    fn base_view(&self, program: Arc<Program>) -> Result<super::Context<'_>, ModuleSourcesFailure> {
        let (source, _) = self
            .paired
            .inventory
            .source_entries()
            .next()
            .ok_or_else(integrity)?;
        let scope = super::definitions::HeaderScope {
            syntax: syntax(&self.paired, source)?,
            lookup: self.scopes[source.index()],
        };
        Ok(self.view(program, scope))
    }
    fn view<'binding>(
        &'binding self,
        program: Arc<Program>,
        scope: super::definitions::HeaderScope<'binding>,
    ) -> super::Context<'binding> {
        super::Context {
            program,
            syntax: scope.syntax,
            lookup: scope.lookup,
            module: Some(self),
            sources: self.catalog.sources.clone(),
            primitives: super::PrimitiveBindings::new(None),
        }
    }
    /// Bind this immutable header snapshot to the actual physical source of one executable root.
    /// Existing source views can remain alive while the writer publishes more headers.
    /// This establishes neither body acceptance nor reusable source-query evidence.
    /// # Errors
    /// Rejects a foreign header writer, unknown roots or broken physical source identities.
    pub fn context_for_code<'binding>(
        &'binding self,
        headers: &ScopedModuleSources<'_>,
        code: AccessCodeId,
    ) -> Result<super::Context<'binding>, ModuleSourcesFailure> {
        if !std::ptr::eq(self, headers.binding.as_ref()) {
            return Err(ModuleSourcesFailure::Access(Failure::ChangedHeader));
        }
        let scope = self.code_scope(code).map_err(|_| integrity())?;
        Ok(self.view(Arc::new(headers.program()), scope))
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
fn field_positions(
    paired: &PairedModuleSources<'_>,
    declarations: &[Option<DeclarationId>],
) -> Box<[Option<FieldPosition>]> {
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
        let authored = inventory
            .declaration_entries()
            .filter(|(_, field)| {
                field.owner() == Some(representation)
                    && field.kind() == SurfaceDeclarationKind::Field
            })
            .collect::<Vec<_>>();
        for (position, (field, _)) in authored.into_iter().enumerate() {
            fields[paired.contracts.representative(field).index()] = Some(FieldPosition {
                record,
                index: position,
            });
        }
    }
    for (id, _) in inventory.declaration_entries() {
        fields[id.index()] = fields[paired.contracts.representative(id).index()];
    }
    fields.into()
}
pub(super) fn integrity() -> ModuleSourcesFailure {
    ModuleSourcesFailure::Access(Failure::InconsistentSyntax)
}
pub(super) fn syntax<'a>(
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
pub(super) fn symbol(
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
