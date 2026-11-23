//! Source-local names, physical headers and progressive metadata over one declaration writer.
use super::{
    CodeSource, Failure, ModuleSourcesFailure, PairedModuleSources,
    headers::{self, Header, RecordHeader},
    projection,
};
use crate::{SurfaceDeclarationId, SurfaceDeclarationKind, SurfaceSourceId};
use nocter_declarations::value_access::{
    BehaviorOwner, DeclarationId, HeaderPublication, Program, Reservations, ScopeId,
};
use nocter_model::{AccessCodeId, AccessFieldId, NominalTypeId, Symbol};
use nocter_syntax::{BoundSyntax, SyntaxToken};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

mod catalog;
mod members;
pub use members::{ComparisonFamily, ConversionSelection, TypeMember, TypeMemberKind};
mod normalization;
mod primitives;
mod record_views;
mod reuse;
pub use reuse::ModuleQuery;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnumVariant {
    pub id: nocter_model::VariantId,
    pub accessible: bool,
}

/// Closed source names and a progressive header writer. Reservation alone grants no typed metadata;
/// selected header publication grants no checked-body, primitive-authority or accepted-Unit proof.
#[derive(Debug)]
pub struct ScopedModuleSources<'syntax> {
    headers: HeaderPublication,
    primitives: super::PrimitiveBindings,
    binding: Arc<ModuleSourceBinding<'syntax>>,
}
/// Closed headers and exact current sources; `query` captures separate reusable source evidence.
#[derive(Debug)]
pub struct ModuleContext<'syntax> {
    program: Arc<Program>,
    primitives: super::PrimitiveBindings,
    binding: Arc<ModuleSourceBinding<'syntax>>,
    dependencies: Option<super::BodySources>,
}
impl ModuleContext<'_> {
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }
    /// The exact authored declaration tokens mapped to canonical code identities by pairing.
    pub fn code_origins(
        &self,
    ) -> impl Iterator<
        Item = Result<
            (
                AccessCodeId,
                nocter_compile_input::ModuleSourceKind,
                nocter_source_index::SourceOrigin,
            ),
            super::SourceMismatch,
        >,
    > + '_ {
        let inventory = &self.binding.paired.inventory;
        inventory
            .declaration_entries()
            .filter_map(|(id, declaration)| {
                let Some(DeclarationId::Code(code)) = self.binding.declarations[id.index()] else {
                    return None;
                };
                let source = &inventory.sources()[declaration.source().index()];
                let nocter_syntax::SyntaxOrigin::Token(token) = declaration.entity_origin() else {
                    return None;
                };
                Some(
                    nocter_source_index::SourceOrigin::from_token(source.syntax(), token)
                        .map(|origin| (code, source.kind(), origin))
                        .map_err(|_| super::SourceMismatch::InconsistentSyntax),
                )
            })
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
        self.binding
            .exports
            .lookup(&self.program, module, caller, namespace, name)
    }
    /// Open the closed metadata in this exact source binding.
    /// # Errors
    /// Rejects inconsistent physical source metadata; grants no body acceptance.
    pub fn current(&self) -> Result<super::Context<'_>, ModuleSourcesFailure> {
        self.binding
            .base_view(Arc::clone(&self.program), self.primitives.clone())
    }
}
/// One immutable physical-source/semantic-slot pairing, retained independently of header writes.
/// No caller can construct it from separately supplied syntax, scopes or declaration identities.
#[derive(Debug)]
pub struct ModuleSourceBinding<'syntax> {
    paired: PairedModuleSources<'syntax>,
    scopes: Box<[ScopeId]>,
    declaration_scopes: Box<[ScopeId]>,
    members: members::Members,
    declarations: Box<[Option<DeclarationId>]>,
    fields: Box<[Option<AccessFieldId>]>,
    catalog: catalog::Catalog,
    exports: super::module_imports::ModuleExports,
    lexical: super::module_imports::LexicalScopes,
}
impl ModuleSourceBinding<'_> {
    pub(super) const fn target(&self) -> nocter_model::CompilationTarget {
        self.paired.inventory.surface.target()
    }
}
#[derive(Debug)]
pub(super) enum ParsedHeader {
    Code(Header),
    Record(RecordHeader),
    Enumeration(headers::EnumHeader),
    Alias(headers::AliasHeader),
    Test {
        name: SyntaxToken,
        declaration: nocter_syntax::NodeId,
        body: nocter_syntax::NodeId,
    },
}
impl ParsedHeader {
    fn executable_source(&self) -> Option<CodeSource> {
        match self {
            Self::Code(header)
                if header.body.is_some()
                    || header.kind == nocter_declarations::value_access::CodeKind::Primitive =>
            {
                Some(CodeSource::from_header(header))
            }
            Self::Test {
                name,
                declaration,
                body,
            } => Some(CodeSource::test(*name, *declaration, *body)),
            _ => None,
        }
    }
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
    pub fn reserve_names(mut self) -> Result<ScopedModuleSources<'syntax>, ModuleSourcesFailure> {
        self.inventory
            .audiences
            .constrain_structural_grants(&self.inventory.surface, &self.contracts)
            .map_err(ModuleSourcesFailure::Access)?;
        let primitive_roles = std::mem::take(&mut self.inventory.primitive_roles);
        let inventory = &self.inventory;
        let parsed = collect_headers(&self)?;
        let mut reservations = Reservations::with_symbols(inventory.surface.symbol_handle());
        let scopes = source_scopes(inventory, &mut reservations)?;
        let mut declarations = vec![None; inventory.declarations().len()];
        let members = reserve_canonical(
            &self,
            &parsed,
            &scopes,
            &mut reservations,
            &mut declarations,
        )?;
        for (id, _) in inventory.declaration_entries() {
            declarations[id.index()] = declarations[self.contracts.representative(id).index()];
        }
        let declaration_scopes: Box<[_]> = inventory
            .declaration_entries()
            .map(|(raw, declaration)| {
                members
                    .get(&raw)
                    .map_or(scopes[declaration.source().index()], |member| member.lookup)
            })
            .collect();
        bind_sources(&self, &scopes, &declarations, &mut reservations)?;
        let body_scopes = parsed
            .iter()
            .filter_map(|(raw, header)| {
                if let ParsedHeader::Code(header) = header
                    && let Some(member) = members.get(raw)
                {
                    return header.body.map(|body| (body, member.lookup));
                }
                None
            })
            .collect();
        let (exports, lexical) = super::module_imports::ModuleExports::bind(
            &self,
            &scopes,
            &body_scopes,
            &declarations,
            &mut reservations,
        )?;
        let field_positions = field_positions(&self, &declarations);
        let catalog = catalog::Catalog::reserve(
            &self,
            parsed,
            &declaration_scopes,
            &declarations,
            &field_positions,
            &mut reservations,
        )?;
        let definitions = reservations.finish().map_err(|violations| {
            project_namespace(&self, &catalog.parsed, &declarations, violations)
        })?;
        let members = members::Members::index(&self, members)?;
        members.constrain_audiences(&mut self)?;
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
        let binding = Arc::new(ModuleSourceBinding {
            paired: self,
            scopes,
            declaration_scopes,
            members,
            declarations: declarations.into(),
            fields,
            catalog,
            exports,
            lexical,
        });
        let primitives = primitives::select(&binding, primitive_roles)?;
        Ok(ScopedModuleSources {
            headers: definitions.into_publication(),
            binding,
            primitives,
        })
    }
}
fn source_scopes(
    inventory: &super::ModuleSources<'_>,
    reservations: &mut Reservations,
) -> Result<Box<[ScopeId]>, ModuleSourcesFailure> {
    inventory
        .source_entries()
        .map(|(source, physical)| {
            let scope = if source.index() == 0 {
                ScopeId::ROOT
            } else {
                reservations.new_scope()
            };
            reservations
                .set_scope_module(scope, physical.module().clone())
                .map_err(|_| integrity())?;
            Ok(scope)
        })
        .collect()
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
        self.binding
            .exports
            .lookup(&self.headers.snapshot(), module, caller, namespace, name)
    }
    /// Exact executable syntax for a definition or registered header count; no body acceptance.
    #[must_use]
    pub fn code_source(&self, code: AccessCodeId) -> Option<&CodeSource> {
        self.binding.catalog.sources.codes.get(&code)
    }
    /// The physical source whose scope applies to the executable root.
    #[must_use]
    pub fn source_for_code(&self, code: AccessCodeId) -> Option<SurfaceSourceId> {
        self.binding
            .catalog
            .owners
            .get(&code)
            .map(|owner| owner.source)
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
        if !self.program().headers_complete() || !self.primitives.all_checked() {
            return Err(integrity());
        }
        let program = self.headers.finish().map_err(|_| integrity())?;
        self.binding
            .members
            .verify_conversions(&self.binding, &program)?;
        self.binding
            .members
            .verify_enum_member_names(&self.binding.paired, &program)?;
        self.binding
            .paired
            .inventory
            .audiences
            .verify_signatures(
                &self.binding.paired.inventory.surface,
                &self.binding.declarations,
                &self.binding.fields,
                &program,
                &self.binding.catalog.parsed,
                &self.binding.declaration_scopes,
            )
            .map_err(ModuleSourcesFailure::Access)?;
        Ok(ModuleContext {
            program: Arc::new(program),
            primitives: self.primitives,
            binding: self.binding,
            dependencies: None,
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
    pub(super) fn type_member(
        &self,
        caller: nocter_source::SourceId,
        owner: BehaviorOwner,
        name: Symbol,
    ) -> Option<TypeMember> {
        self.members.lookup(self, caller, owner, name)
    }

    pub(super) fn comparison(
        &self,
        caller: nocter_source::SourceId,
        owner: BehaviorOwner,
        family: ComparisonFamily,
    ) -> Option<TypeMember> {
        self.members.comparison(self, caller, owner, family)
    }

    pub(super) fn conversion(
        &self,
        caller: nocter_source::SourceId,
        program: &Program,
        owner: BehaviorOwner,
        target: nocter_model::TypeId,
    ) -> ConversionSelection {
        self.members
            .conversion(self, caller, program, owner, target)
    }

    fn declaration_access(
        &self,
        caller: nocter_source::SourceId,
        declaration: SurfaceDeclarationId,
    ) -> Option<bool> {
        let caller = self.physical_source(caller)?;
        let inventory = &self.paired.inventory;
        let physical = inventory.declarations().get(declaration.index())?;
        Some(inventory.audience(declaration).map_or_else(
            || {
                super::module_sources::visible_sources(&self.paired, caller)
                    .any(|source| source == physical.source())
            },
            |audience| audience.contains(inventory.sources()[caller.index()].module()),
        ))
    }

    pub(super) fn enum_variant_access(
        &self,
        caller: nocter_source::SourceId,
        owner: NominalTypeId,
    ) -> Option<bool> {
        self.declaration_access(caller, *self.catalog.enum_owners.get(&owner)?)
    }

    pub(super) fn owns_enum_schema(&self, owner: NominalTypeId) -> bool {
        self.catalog.enum_owners.contains_key(&owner)
    }

    fn physical_source(&self, source: nocter_source::SourceId) -> Option<SurfaceSourceId> {
        self.paired
            .inventory
            .source_entries()
            .find(|(_, physical)| physical.syntax().source() == source)
            .map(|(id, _)| id)
    }

    /// Authored top-level root contracts only. This enumerates promises, not ready API evidence.
    pub fn exports(
        &self,
    ) -> impl Iterator<Item = (SurfaceDeclarationId, DeclarationId, &super::ModuleAudience)> + '_
    {
        self.paired
            .inventory
            .declaration_entries()
            .filter_map(|(raw, declaration)| {
                if !super::module_sources::has_source_name(declaration) {
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
    pub(super) fn header_occurrences(
        &self,
    ) -> impl Iterator<
        Item = Result<
            (
                DeclarationId,
                &ParsedHeader,
                super::definitions::HeaderScope<'_>,
            ),
            super::SourceMismatch,
        >,
    > + '_ {
        self.catalog.parsed.iter().map(|(raw, header)| {
            let source = self.paired.inventory.declarations()[raw.index()].source();
            let id =
                self.declarations[raw.index()].ok_or(super::SourceMismatch::InconsistentSyntax)?;
            let scope = super::definitions::HeaderScope {
                syntax: syntax(&self.paired, source)
                    .map_err(|_| super::SourceMismatch::InconsistentSyntax)?,
                lookup: self.declaration_scopes[raw.index()],
            };
            Ok((id, header, scope))
        })
    }
    pub(super) fn code_scope(
        &self,
        code: AccessCodeId,
    ) -> Result<super::definitions::HeaderScope<'_>, super::SourceMismatch> {
        let owner = self
            .catalog
            .owners
            .get(&code)
            .copied()
            .ok_or(super::SourceMismatch::UnknownSource)?;
        Ok(super::definitions::HeaderScope {
            syntax: syntax(&self.paired, owner.source)
                .map_err(|_| super::SourceMismatch::InconsistentSyntax)?,
            lookup: owner.lookup,
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
            .physical_source(source)
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
        self.base_view(Arc::new(headers.program()), headers.primitives.clone())
    }
    fn base_view(
        &self,
        program: Arc<Program>,
        primitives: super::PrimitiveBindings,
    ) -> Result<super::Context<'_>, ModuleSourcesFailure> {
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
        Ok(self.view(program, scope, primitives))
    }
    fn view<'binding>(
        &'binding self,
        program: Arc<Program>,
        scope: super::definitions::HeaderScope<'binding>,
        primitives: super::PrimitiveBindings,
    ) -> super::Context<'binding> {
        super::Context {
            program,
            syntax: scope.syntax,
            lookup: scope.lookup,
            module: Some(self),
            sources: self.catalog.sources.clone(),
            primitives,
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
        Ok(self.view(
            Arc::new(headers.program()),
            scope,
            headers.primitives.clone(),
        ))
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
    let mut enums = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    for (id, header) in parsed {
        if paired.contracts.representative(*id) != *id {
            continue;
        }
        match (declarations[id.index()], header) {
            (Some(DeclarationId::Code(id)), ParsedHeader::Code(header)) => {
                codes.insert(id, CodeSource::from_header(header));
            }
            (Some(DeclarationId::Nominal(id)), ParsedHeader::Record(header)) => {
                records.insert(id, Arc::clone(&header.source));
            }
            (Some(DeclarationId::Nominal(id)), ParsedHeader::Enumeration(header)) => {
                enums.insert(id, Arc::clone(&header.source));
            }
            (Some(DeclarationId::Alias(id)), ParsedHeader::Alias(header)) => {
                aliases.insert(id, header.source);
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
        &trees, &codes, &records, &enums, &aliases, violations,
    ))
}
fn field_positions(
    paired: &PairedModuleSources<'_>,
    declarations: &[Option<DeclarationId>],
) -> Box<[Option<FieldPosition>]> {
    let inventory = &paired.inventory;
    let mut fields = vec![None; inventory.declarations().len()];
    for (contract, declaration) in inventory.declaration_entries() {
        let Some(DeclarationId::Nominal(record)) = declarations[contract.index()] else {
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
        for (position, (field, _)) in inventory.surface.record_fields(representation).enumerate() {
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
                SurfaceDeclarationKind::Function
                | SurfaceDeclarationKind::PrimitiveFunction
                | SurfaceDeclarationKind::ConstructionFunction => {
                    syntax(paired, declaration.source()).and_then(|s| {
                        headers::function(s, declaration.node())
                            .map(ParsedHeader::Code)
                            .map_err(ModuleSourcesFailure::Access)
                    })
                }
                SurfaceDeclarationKind::InherentMethod
                | SurfaceDeclarationKind::Coercion
                | SurfaceDeclarationKind::Equality
                | SurfaceDeclarationKind::Ordering => syntax(paired, declaration.source())
                    .and_then(|s| {
                        let owner = declaration.owner().ok_or_else(integrity)?;
                        let physical = paired.inventory.declarations()[owner.index()];
                        let project = match declaration.kind() {
                            SurfaceDeclarationKind::Coercion => members::conversion_header,
                            SurfaceDeclarationKind::InherentMethod => members::method_header,
                            _ => members::comparison_header,
                        };
                        project(s, physical.node(), declaration.node())
                            .map(ParsedHeader::Code)
                            .map_err(ModuleSourcesFailure::Access)
                    }),
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
                SurfaceDeclarationKind::Enum => {
                    syntax(paired, declaration.source()).and_then(|s| {
                        headers::enumeration(s, declaration.node())
                            .map(ParsedHeader::Enumeration)
                            .map_err(ModuleSourcesFailure::Access)
                    })
                }
                SurfaceDeclarationKind::TypeAlias => {
                    syntax(paired, declaration.source()).and_then(|s| {
                        headers::alias(s, declaration.node())
                            .map(ParsedHeader::Alias)
                            .map_err(ModuleSourcesFailure::Access)
                    })
                }
                SurfaceDeclarationKind::Test => {
                    syntax(paired, declaration.source()).and_then(|s| {
                        let name = declaration.name().ok_or_else(integrity)?;
                        let body = nocter_syntax::declaration_body(s.tree(), declaration.node())
                            .ok_or_else(integrity)?;
                        Ok(ParsedHeader::Test {
                            name,
                            declaration: declaration.node(),
                            body,
                        })
                    })
                }
                SurfaceDeclarationKind::Field
                | SurfaceDeclarationKind::Variant
                | SurfaceDeclarationKind::TypeBehavior
                | SurfaceDeclarationKind::PrimitiveType
                | SurfaceDeclarationKind::StructuralLiteralGrant => {
                    return None;
                }
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
) -> Result<BTreeMap<SurfaceDeclarationId, members::MemberOccurrence>, ModuleSourcesFailure> {
    let mut ordered = Vec::new();
    for (id, header) in parsed {
        if paired.contracts.representative(*id) != *id {
            continue;
        }
        let declaration = paired.inventory.declarations()[id.index()];
        let (record, name) = match header {
            ParsedHeader::Code(header) => (false, Some(header.name)),
            ParsedHeader::Record(header) => (true, Some(header.source.name)),
            ParsedHeader::Enumeration(header) => (true, Some(header.source.name)),
            ParsedHeader::Alias(header) => (false, Some(header.source.name)),
            ParsedHeader::Test { .. } => (false, None),
        };
        ordered.push((
            !record,
            declaration.source(),
            name.map(|name| symbol(paired, declaration.source(), name))
                .transpose()?,
            *id,
        ));
    }
    ordered.sort_unstable();
    let first_code = ordered.partition_point(|(code, ..)| !code);
    for &(_, _, name, id) in &ordered[..first_code] {
        let definition = paired.contracts.representation(id).unwrap_or(id);
        let physical = paired.inventory.declarations()[definition.index()];
        let scope = scopes[physical.source().index()];
        let name = name.ok_or_else(integrity)?;
        let nominal = match parsed.get(&definition).ok_or_else(integrity)? {
            ParsedHeader::Record(header) => {
                reservations.reserve_record_in(scope, name, header.fields.len())
            }
            ParsedHeader::Enumeration(header) if !header.opaque => {
                reservations.reserve_enum_in(scope, name, header.source.variants.len())
            }
            _ => return Err(integrity()),
        }
        .map_err(|_| integrity())?;
        declarations[id.index()] = Some(DeclarationId::Nominal(nominal));
    }
    let members = members::occurrences(paired, declarations, scopes, reservations)?;
    let mut test_names = BTreeMap::new();
    for &(_, _, name, id) in &ordered[first_code..] {
        let declaration = paired.inventory.declarations()[id.index()];
        let semantic = match parsed.get(&id).ok_or_else(integrity)? {
            ParsedHeader::Code(header) => {
                let name = name.ok_or_else(integrity)?;
                let count = header.parameters.len();
                match members.get(&id) {
                    Some(member) => reservations.reserve_type_member(member.lookup, name, count),
                    None => {
                        reservations.reserve_in(scopes[declaration.source().index()], name, count)
                    }
                }
                .map(DeclarationId::Code)
            }
            ParsedHeader::Record(_) | ParsedHeader::Enumeration(_) => return Err(integrity()),
            ParsedHeader::Alias(_) => reservations
                .reserve_alias(
                    scopes[declaration.source().index()],
                    name.ok_or_else(integrity)?,
                )
                .map(DeclarationId::Alias),
            ParsedHeader::Test { name, .. } => {
                let module = paired.inventory.sources()[declaration.source().index()].module();
                let spelling = symbol(paired, declaration.source(), *name)?;
                if let Some(first) = test_names.insert((module, spelling), *name) {
                    let violation = crate::NamespaceViolation::name_collision(
                        nocter_syntax::SyntaxOrigin::Token(first),
                        nocter_syntax::SyntaxOrigin::Token(*name),
                    );
                    let diagnostic = crate::NamespaceDiagnostic::project_surface(
                        violation,
                        &paired.inventory.surface,
                    )
                    .map_err(|_| integrity())?;
                    return Err(ModuleSourcesFailure::Access(Failure::Namespace(Box::new(
                        diagnostic,
                    ))));
                }
                reservations
                    .reserve_test(scopes[declaration.source().index()], spelling)
                    .map(DeclarationId::Code)
            }
        }
        .map_err(|_| integrity())?;
        declarations[id.index()] = Some(semantic);
    }
    Ok(members)
}
fn bind_sources(
    paired: &PairedModuleSources<'_>,
    scopes: &[ScopeId],
    declarations: &[Option<DeclarationId>],
    reservations: &mut Reservations,
) -> Result<(), ModuleSourcesFailure> {
    let mut authored = BTreeMap::<SurfaceSourceId, Vec<(Symbol, DeclarationId)>>::new();
    for (id, declaration) in paired.inventory.declaration_entries() {
        if !super::module_sources::has_source_name(declaration) {
            continue;
        }
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
        let visible: BTreeSet<_> = super::module_sources::visible_sources(paired, source).collect();
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
