//! Exact source evidence shared by declaration and checking consumers, without evaluation.
use super::Context;
use nocter_declarations::value_access::{DeclarationId, HeaderPosition, ParameterBinding, Program};
use nocter_model::{AccessCodeId, NominalTypeId, TypeAliasId, ValueCodeRoot, ValueExpression};
use nocter_syntax::{BodySyntaxLocator, BodySyntaxProjection, NodeId, SyntaxOrigin, SyntaxToken};
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

mod dependencies;
mod expressions;
mod recipes;
mod registration;
pub use dependencies::{BodyDependencies, BodySourceGraph, BodySourceNode};
pub use expressions::BodyExpressions;
pub(super) use recipes::Recipes;
pub(super) use registration::requires_array_value;

#[derive(Clone, Debug)]
pub(super) struct Sources {
    pub codes: Arc<BTreeMap<AccessCodeId, CodeSource>>,
    pub records: Arc<BTreeMap<NominalTypeId, Arc<RecordSource>>>,
    pub enums: Arc<BTreeMap<NominalTypeId, Arc<EnumSource>>>,
    pub aliases: Arc<BTreeMap<TypeAliasId, AliasSource>>,
    expressions: Arc<BTreeMap<NodeId, AccessCodeId>>,
}
impl Sources {
    pub(super) fn new(
        codes: BTreeMap<AccessCodeId, CodeSource>,
        records: BTreeMap<NominalTypeId, Arc<RecordSource>>,
    ) -> Result<Self, super::Failure> {
        Self::from_module(codes, records, BTreeMap::new(), BTreeMap::new())
    }

    pub(super) fn from_module(
        codes: BTreeMap<AccessCodeId, CodeSource>,
        records: BTreeMap<NominalTypeId, Arc<RecordSource>>,
        enums: BTreeMap<NominalTypeId, Arc<EnumSource>>,
        aliases: BTreeMap<TypeAliasId, AliasSource>,
    ) -> Result<Self, super::Failure> {
        let mut expressions = BTreeMap::new();
        for (&id, source) in &codes {
            if let CodeSourceKind::Expression(body) = source.kind
                && expressions.insert(body, id).is_some()
            {
                return Err(super::Failure::InconsistentSyntax);
            }
        }
        Ok(Self {
            codes: Arc::new(codes),
            records: Arc::new(records),
            enums: Arc::new(enums),
            aliases: Arc::new(aliases),
            expressions: Arc::new(expressions),
        })
    }
    pub(super) fn expression(&self, node: NodeId) -> Option<AccessCodeId> {
        self.expressions.get(&node).copied()
    }
    pub(super) fn expression_ids(&self) -> impl Iterator<Item = AccessCodeId> + '_ {
        self.expressions.values().copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMismatch {
    UnknownSource,
    ChangedAuthority,
    ChangedSource,
    InconsistentSyntax,
    InconsistentDependencies,
}

#[derive(Clone, Debug)]
pub(super) struct FieldSource {
    pub name: SyntaxToken,
    pub ty: NodeId,
}
#[derive(Clone, Debug)]
pub(super) struct RecordSource {
    pub name: SyntaxToken,
    pub fields: Box<[FieldSource]>,
}
#[derive(Clone, Debug)]
pub(super) struct EnumSource {
    pub name: SyntaxToken,
    pub variants: Box<[VariantSource]>,
}
#[derive(Clone, Debug)]
pub(super) struct VariantSource {
    pub name: SyntaxToken,
    pub fields: Box<[FieldSource]>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct AliasSource {
    pub name: SyntaxToken,
    pub ty: NodeId,
}

#[derive(Clone, Debug)]
pub struct CodeSource {
    kind: CodeSourceKind,
}
#[derive(Clone, Debug)]
enum CodeSourceKind {
    Declaration {
        header: CodeHeaderSource,
        body: Option<NodeId>,
    },
    Expression(NodeId),
}
#[derive(Clone, Debug)]
struct CodeHeaderSource {
    name: SyntaxToken,
    deferred_modifier: Option<NodeId>,
    generic_parameters: Option<NodeId>,
    parameters: Box<[ParameterSource<SyntaxOrigin>]>,
    result: NodeId,
}

#[derive(Clone, Debug)]
pub(super) struct ParameterSource<Origin> {
    pub origin: Origin,
    pub binding: ParameterBinding<Origin>,
    pub ty: Origin,
}
impl<Origin> ParameterSource<Origin> {
    fn try_map<T, E>(
        &self,
        mut map: impl FnMut(&Origin) -> Result<T, E>,
    ) -> Result<ParameterSource<T>, E> {
        Ok(ParameterSource {
            origin: map(&self.origin)?,
            binding: self.binding.try_map(&mut map)?,
            ty: map(&self.ty)?,
        })
    }
}
impl ParameterSource<SyntaxOrigin> {
    pub(super) const fn named(token: SyntaxToken, ty: NodeId) -> Self {
        let origin = SyntaxOrigin::Token(token);
        Self {
            origin,
            binding: ParameterBinding::Name(origin),
            ty: SyntaxOrigin::Node(ty),
        }
    }

    pub(super) fn names(&self) -> impl Iterator<Item = SyntaxToken> {
        self.binding.names().filter_map(|origin| match origin {
            SyntaxOrigin::Token(token) => Some(*token),
            SyntaxOrigin::Node(_) => None,
        })
    }

    pub(super) const fn type_node(&self) -> Option<NodeId> {
        match self.ty {
            SyntaxOrigin::Node(node) => Some(node),
            SyntaxOrigin::Token(_) => None,
        }
    }
}

impl CodeSource {
    fn declaration(header: CodeHeaderSource, body: Option<NodeId>) -> Self {
        Self {
            kind: CodeSourceKind::Declaration { header, body },
        }
    }
    pub(in crate::access_declarations) fn from_header(header: &super::headers::Header) -> Self {
        Self::declaration(
            CodeHeaderSource {
                name: header.name,
                deferred_modifier: header.deferred_modifier,
                generic_parameters: header.generics.as_ref().map(|list| list.origin),
                parameters: header
                    .parameters
                    .iter()
                    .map(|parameter| parameter.source.clone())
                    .collect(),
                result: header.result_syntax,
            },
            header.body,
        )
    }
    pub(in crate::access_declarations) fn expression(body: NodeId) -> Self {
        Self {
            kind: CodeSourceKind::Expression(body),
        }
    }
    pub(in crate::access_declarations) fn test(
        name: SyntaxToken,
        declaration: NodeId,
        body: NodeId,
    ) -> Self {
        Self::declaration(
            CodeHeaderSource {
                name,
                deferred_modifier: None,
                generic_parameters: None,
                parameters: Box::new([]),
                result: declaration,
            },
            Some(body),
        )
    }
    pub(in crate::access_declarations) fn destructor(
        keyword: SyntaxToken,
        declaration: NodeId,
        body: NodeId,
    ) -> Self {
        let origin = SyntaxOrigin::Token(keyword);
        Self::declaration(
            CodeHeaderSource {
                name: keyword,
                deferred_modifier: None,
                generic_parameters: None,
                parameters: Box::new([ParameterSource {
                    origin,
                    binding: ParameterBinding::Name(origin),
                    ty: origin,
                }]),
                result: declaration,
            },
            Some(body),
        )
    }
    fn header(&self) -> Option<&CodeHeaderSource> {
        match &self.kind {
            CodeSourceKind::Declaration { header, .. } => Some(header),
            CodeSourceKind::Expression(_) => None,
        }
    }
    pub(super) fn name(&self) -> Option<SyntaxToken> {
        self.header().map(|header| header.name)
    }
    /// Source position of the callable's authored deferred execution mode.
    #[must_use]
    pub fn deferred_modifier(&self) -> Option<NodeId> {
        self.header()?.deferred_modifier
    }
    /// The authored declaration anchor or the exact anonymous expression root.
    #[must_use]
    pub fn origin(&self) -> SyntaxOrigin {
        match &self.kind {
            CodeSourceKind::Declaration { header, .. } => SyntaxOrigin::Token(header.name),
            CodeSourceKind::Expression(body) => SyntaxOrigin::Node(*body),
        }
    }
    pub(super) fn parameters(&self) -> &[ParameterSource<SyntaxOrigin>] {
        self.header().map_or(&[], |header| &header.parameters)
    }

    /// The complete authored compile-time parameter list, when one exists.
    #[must_use]
    pub fn generic_parameter_origin(&self) -> Option<NodeId> {
        self.header()?.generic_parameters
    }

    /// The whole input pattern, or one named leaf in written order.
    #[must_use]
    pub fn parameter_origin(&self, input: usize, leaf: Option<usize>) -> Option<SyntaxOrigin> {
        let parameter = self.parameters().get(input)?;
        match leaf {
            None => Some(parameter.origin),
            Some(leaf) => parameter.binding.names().nth(leaf).copied(),
        }
    }

    /// The authored type annotation of an input, without resolving it.
    #[must_use]
    pub fn parameter_type(&self, position: usize) -> Option<NodeId> {
        self.parameters().get(position)?.type_node()
    }
    /// The authored annotation or source anchor for an implicit input type.
    #[must_use]
    pub fn parameter_type_origin(&self, position: usize) -> Option<SyntaxOrigin> {
        Some(self.parameters().get(position)?.ty)
    }
    /// The result annotation, or the expression requiring a usize result for anonymous code.
    #[must_use]
    pub fn result_origin(&self) -> NodeId {
        match &self.kind {
            CodeSourceKind::Declaration { header, .. } => header.result,
            CodeSourceKind::Expression(body) => *body,
        }
    }
    /// The actual executable root; bodyless declarations provide none.
    #[must_use]
    pub const fn body(&self) -> Option<NodeId> {
        match &self.kind {
            CodeSourceKind::Declaration { body, .. } => *body,
            CodeSourceKind::Expression(body) => Some(*body),
        }
    }
}

/// Exact bytes and header lineage, never a source lookup or semantic authority of their own.
#[derive(Clone, Debug)]
struct Evidence {
    header: Arc<Program>,
    bytes: Arc<[u8]>,
}
impl Evidence {
    fn capture(context: &Context<'_>, bytes: &[u8]) -> Self {
        Self {
            header: context.program_handle(),
            bytes: bytes.into(),
        }
    }
    fn bind<'source, T>(
        &self,
        context: &Context<'_>,
        locate: impl FnOnce() -> Result<(T, &'source [u8]), SourceMismatch>,
    ) -> Result<T, SourceMismatch> {
        if !context.program().preserves_headers(&self.header) {
            return Err(SourceMismatch::ChangedAuthority);
        }
        let (source, bytes) = locate()?;
        if bytes != self.bytes.as_ref() {
            return Err(SourceMismatch::ChangedSource);
        }
        Ok(source)
    }
    fn retain(&mut self, source: &Self) -> Result<(), SourceMismatch> {
        if self.bytes != source.bytes {
            return Err(SourceMismatch::ChangedSource);
        }
        if source.header.preserves_headers(&self.header) {
            self.header = source.header.clone();
        } else if !self.header.preserves_headers(&source.header) {
            return Err(SourceMismatch::ChangedAuthority);
        }
        Ok(())
    }
}

/// A source-neutral body-owner/bytes/authority pairing captured only from a current context.
/// This evidence grants neither a ready contract nor body acceptance.
#[derive(Clone, Debug)]
pub struct BodySource {
    owner: AccessCodeId,
    root: BodyRoot,
    evidence: Evidence,
}
#[derive(Clone, Copy, Debug)]
enum BodyRoot {
    Declaration,
    Expression {
        slot: u32,
        locator: BodySyntaxLocator,
    },
}
impl BodySource {
    #[must_use]
    pub const fn owner(&self) -> AccessCodeId {
        self.owner
    }
    /// The selected executable root, independently of its enclosing source owner.
    #[must_use]
    pub const fn code(&self) -> ValueCodeRoot {
        match self.root {
            BodyRoot::Declaration => ValueCodeRoot::Declaration(self.owner),
            BodyRoot::Expression { slot, .. } => ValueCodeRoot::Expression(ValueExpression {
                owner: self.owner,
                slot,
            }),
        }
    }
    /// Locate the same body in an exact descendant of its original declaration authority.
    /// # Errors
    /// Rejects a foreign authority, a changed body or inconsistent source metadata.
    pub fn bind<'context>(
        &self,
        context: &'context Context<'_>,
    ) -> Result<Cow<'context, CodeSource>, SourceMismatch> {
        let source = self.verify_owner(context)?;
        match self.root {
            BodyRoot::Declaration => Ok(Cow::Borrowed(source)),
            BodyRoot::Expression { locator, .. } => {
                let projection = BodySyntaxProjection::for_root(
                    context.for_code(self.owner)?.syntax().tree(),
                    source.body().ok_or(SourceMismatch::UnknownSource)?,
                )
                .ok_or(SourceMismatch::InconsistentSyntax)?;
                let Some(SyntaxOrigin::Node(body)) = projection.resolve(locator) else {
                    return Err(SourceMismatch::InconsistentSyntax);
                };
                Ok(Cow::Owned(CodeSource::expression(body)))
            }
        }
    }
    fn verify_owner<'context>(
        &self,
        context: &'context Context<'_>,
    ) -> Result<&'context CodeSource, SourceMismatch> {
        self.evidence.bind(context, || body(context, self.owner))
    }
}

/// One declaration-owned type annotation, including an unready record's field annotation.
/// The source root is not executable code and grants no normalized type or value.
#[derive(Clone, Debug)]
pub struct HeaderSource {
    declaration: DeclarationId,
    position: HeaderPosition,
    evidence: Evidence,
}
impl HeaderSource {
    #[must_use]
    pub const fn declaration(&self) -> DeclarationId {
        self.declaration
    }
    #[must_use]
    pub const fn position(&self) -> HeaderPosition {
        self.position
    }
    /// Resolve this exact annotation to its current syntax node without type lookup or parsing.
    /// # Errors
    /// Rejects changed authority/source or a missing/inconsistent current annotation.
    pub fn bind(&self, context: &Context<'_>) -> Result<NodeId, SourceMismatch> {
        self.evidence
            .bind(context, || header(context, self.declaration, self.position))
    }
}

impl Context<'_> {
    /// Capture a body dependency without permitting a caller-supplied owner/bytes join.
    /// # Errors
    /// Rejects an unknown body or an inconsistent current source pairing.
    pub fn body_source(&self, owner: AccessCodeId) -> Result<BodySource, SourceMismatch> {
        let (_, bytes) = body(self, owner)?;
        Ok(BodySource {
            owner,
            root: BodyRoot::Declaration,
            evidence: Evidence::capture(self, bytes),
        })
    }
    /// Capture one raw type-annotation root independently of its contract's readiness.
    /// # Errors
    /// Rejects a non-type position, an unknown slot or an inconsistent current source pairing.
    pub fn header_source(
        &self,
        declaration: DeclarationId,
        position: HeaderPosition,
    ) -> Result<HeaderSource, SourceMismatch> {
        let (_, bytes) = header(self, declaration, position)?;
        Ok(HeaderSource {
            declaration,
            position,
            evidence: Evidence::capture(self, bytes),
        })
    }
}

/// A union of exact body dependencies. Conflicting evidence never replaces an existing guard.
#[derive(Clone, Debug, Default)]
pub struct BodySources {
    bodies: BTreeMap<AccessCodeId, BodySource>,
}
impl BodySources {
    /// Retain the strongest compatible header prefix for this source owner.
    /// # Errors
    /// Rejects conflicting bytes or incomparable declaration authorities without mutation.
    pub fn insert(&mut self, source: &BodySource) -> Result<(), SourceMismatch> {
        use std::collections::btree_map::Entry;
        match self.bodies.entry(source.owner) {
            Entry::Vacant(entry) => {
                entry.insert(source.clone());
            }
            Entry::Occupied(mut entry) => {
                entry.get_mut().evidence.retain(&source.evidence)?;
            }
        }
        Ok(())
    }
    /// Whether this evidence retains the selected source owner, independently of byte validation.
    #[must_use]
    pub fn contains(&self, owner: AccessCodeId) -> bool {
        self.bodies.contains_key(&owner)
    }
    /// Verify source dependencies without lookup, checking or evaluation of their contents.
    /// # Errors
    /// Reports the first changed authority/body or inconsistent current source pairing.
    pub fn verify(&self, context: &Context<'_>) -> Result<(), SourceMismatch> {
        for source in self.bodies.values() {
            source.verify_owner(context)?;
        }
        Ok(())
    }
}

fn body<'context, 'source>(
    context: &'context Context<'source>,
    owner: AccessCodeId,
) -> Result<(&'context CodeSource, &'source [u8]), SourceMismatch> {
    let source = context
        .code_source(owner)
        .ok_or(SourceMismatch::UnknownSource)?;
    Ok((
        source,
        bytes(
            context.for_code(owner)?.syntax(),
            source.body().ok_or(SourceMismatch::UnknownSource)?,
        )?,
    ))
}
fn header<'source>(
    context: &Context<'source>,
    declaration: DeclarationId,
    position: HeaderPosition,
) -> Result<(NodeId, &'source [u8]), SourceMismatch> {
    let Some(SyntaxOrigin::Node(node)) = header_origin(
        &context.sources.codes,
        &context.sources.records,
        &context.sources.enums,
        &context.sources.aliases,
        declaration,
        position,
        true,
    ) else {
        return Err(SourceMismatch::UnknownSource);
    };
    Ok((
        node,
        bytes(context.syntax_for_origin(SyntaxOrigin::Node(node))?, node)?,
    ))
}
pub(super) fn header_origin(
    sources: &BTreeMap<AccessCodeId, CodeSource>,
    records: &BTreeMap<NominalTypeId, Arc<RecordSource>>,
    enums: &BTreeMap<NominalTypeId, Arc<EnumSource>>,
    aliases: &BTreeMap<TypeAliasId, AliasSource>,
    declaration: DeclarationId,
    position: HeaderPosition,
    typed: bool,
) -> Option<SyntaxOrigin> {
    Some(match declaration {
        DeclarationId::Code(id) => {
            let source = sources.get(&id)?;
            let s = source.header()?;
            match position {
                HeaderPosition::Name => SyntaxOrigin::Token(s.name),
                HeaderPosition::Parameter(i) if typed => {
                    SyntaxOrigin::Node(source.parameter_type(i)?)
                }
                HeaderPosition::Parameter(i) => source.parameter_origin(i, None)?,
                HeaderPosition::ParameterName { input, leaf } => {
                    source.parameter_origin(input, Some(leaf))?
                }
                HeaderPosition::Result => SyntaxOrigin::Node(s.result),
                HeaderPosition::Field(_)
                | HeaderPosition::Variant(_)
                | HeaderPosition::VariantField { .. } => return None,
            }
        }
        DeclarationId::Nominal(id) => {
            if let Some(record) = records.get(&id) {
                match position {
                    HeaderPosition::Name => SyntaxOrigin::Token(record.name),
                    HeaderPosition::Field(i) if typed => {
                        SyntaxOrigin::Node(record.fields.get(i)?.ty)
                    }
                    HeaderPosition::Field(i) => SyntaxOrigin::Token(record.fields.get(i)?.name),
                    _ => return None,
                }
            } else {
                let schema = enums.get(&id)?;
                match position {
                    HeaderPosition::Name => SyntaxOrigin::Token(schema.name),
                    HeaderPosition::Variant(index) => {
                        SyntaxOrigin::Token(schema.variants.get(index)?.name)
                    }
                    HeaderPosition::VariantField { variant, field } => {
                        let source = schema.variants.get(variant)?.fields.get(field)?;
                        if typed {
                            SyntaxOrigin::Node(source.ty)
                        } else {
                            SyntaxOrigin::Token(source.name)
                        }
                    }
                    _ => return None,
                }
            }
        }
        DeclarationId::Alias(id) => {
            let alias = aliases.get(&id)?;
            match position {
                HeaderPosition::Name => SyntaxOrigin::Token(alias.name),
                HeaderPosition::Result => SyntaxOrigin::Node(alias.ty),
                _ => return None,
            }
        }
    })
}

fn bytes(syntax: nocter_syntax::BoundSyntax<'_>, root: NodeId) -> Result<&[u8], SourceMismatch> {
    let range = syntax
        .tree()
        .node(root)
        .ok_or(SourceMismatch::InconsistentSyntax)?
        .range();
    Ok(syntax
        .text_at(range)
        .ok_or(SourceMismatch::InconsistentSyntax)?
        .as_bytes())
}

#[cfg(test)]
mod tests;
