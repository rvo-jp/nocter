use std::collections::BTreeMap;
use std::fmt;

use nocter_checking::{
    AssociatedTypeCompletionError, CheckedOperation, CheckedProgram, ConstructionCompletionError,
    EnumPatternCompletionError, MemberCompletionContext, MemberCompletionError,
    MemberCompletionTarget, NameTarget, ReceiverPreparation, StructuralFieldCompletionError,
    TypedBodyInterruptionKind,
};
use nocter_declarations::{DeclarationGraph, ExportedEntity, NominalShape};
use nocter_model::{BodyId, BodyScopeId, BorrowCapability, Symbol};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole};

use crate::AnalysisSnapshot;
use crate::presentation::visible_spelling::VisibleSpellings;
use crate::presentation::{prepared_presentation, presentation};
use crate::semantic::SemanticAuthority;
use crate::source_context::{SourceContext, SourceContextError};

mod associated_types;
mod automatic_imports;
mod construction;
mod enum_patterns;
mod keywords;
mod structural_fields;

/// One compiler-selected name visible at an exact source position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCompletion {
    label: Box<str>,
    kind: SemanticCompletionKind,
    detail: Option<Box<str>>,
    additional_edits: Box<[SemanticCompletionEdit]>,
    automatic_import: Option<Box<str>>,
}

impl SemanticCompletion {
    fn new(
        label: impl Into<Box<str>>,
        kind: SemanticCompletionKind,
        detail: Option<Box<str>>,
    ) -> Self {
        Self {
            label: label.into(),
            kind,
            detail,
            additional_edits: Box::new([]),
            automatic_import: None,
        }
    }

    fn with_additional_edit(mut self, edit: SemanticCompletionEdit) -> Self {
        self.additional_edits = Box::new([edit]);
        self
    }

    fn with_automatic_import(mut self, path: impl Into<Box<str>>) -> Self {
        self.automatic_import = Some(path.into());
        self
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> SemanticCompletionKind {
        self.kind
    }

    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    #[must_use]
    pub const fn additional_edits(&self) -> &[SemanticCompletionEdit] {
        &self.additional_edits
    }

    /// Returns the exact compiler-selected import route for an automatic-import candidate.
    #[must_use]
    pub fn automatic_import(&self) -> Option<&str> {
        self.automatic_import.as_deref()
    }
}

/// One protocol-independent source edit attached to a semantic completion candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCompletionEdit {
    range: TextRange,
    new_text: Box<str>,
}

impl SemanticCompletionEdit {
    #[must_use]
    pub fn new(range: TextRange, new_text: impl Into<Box<str>>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }

    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    #[must_use]
    pub fn new_text(&self) -> &str {
        &self.new_text
    }
}

/// Protocol-independent completion categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticCompletionKind {
    Module,
    Struct,
    Enum,
    Type,
    Interface,
    Function,
    Constructor,
    EnumMember,
    Field,
    Method,
    Parameter,
    Variable,
    Constant,
    Keyword,
}

/// An internal inconsistency while deriving completion from immutable compiler state.
#[derive(Debug)]
pub enum SemanticCompletionError {
    SourceContext(SourceContextError),
    Member(MemberCompletionError),
    Construction(ConstructionCompletionError),
    StructuralField(StructuralFieldCompletionError),
    EnumPattern(EnumPatternCompletionError),
    AssociatedType(AssociatedTypeCompletionError),
    AutomaticImport(automatic_imports::AutomaticImportError),
}

impl fmt::Display for SemanticCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceContext(error) => error.fmt(formatter),
            Self::Member(error) => error.fmt(formatter),
            Self::Construction(error) => error.fmt(formatter),
            Self::StructuralField(error) => error.fmt(formatter),
            Self::EnumPattern(error) => error.fmt(formatter),
            Self::AssociatedType(error) => error.fmt(formatter),
            Self::AutomaticImport(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceContext(error) => Some(error),
            Self::Member(error) => Some(error),
            Self::Construction(error) => Some(error),
            Self::StructuralField(error) => Some(error),
            Self::EnumPattern(error) => Some(error),
            Self::AssociatedType(error) => Some(error),
            Self::AutomaticImport(error) => Some(error),
        }
    }
}

impl From<SourceContextError> for SemanticCompletionError {
    fn from(error: SourceContextError) -> Self {
        Self::SourceContext(error)
    }
}

impl From<MemberCompletionError> for SemanticCompletionError {
    fn from(error: MemberCompletionError) -> Self {
        Self::Member(error)
    }
}

impl From<ConstructionCompletionError> for SemanticCompletionError {
    fn from(error: ConstructionCompletionError) -> Self {
        Self::Construction(error)
    }
}

impl From<StructuralFieldCompletionError> for SemanticCompletionError {
    fn from(error: StructuralFieldCompletionError) -> Self {
        Self::StructuralField(error)
    }
}

impl From<EnumPatternCompletionError> for SemanticCompletionError {
    fn from(error: EnumPatternCompletionError) -> Self {
        Self::EnumPattern(error)
    }
}

impl From<AssociatedTypeCompletionError> for SemanticCompletionError {
    fn from(error: AssociatedTypeCompletionError) -> Self {
        Self::AssociatedType(error)
    }
}

impl From<automatic_imports::AutomaticImportError> for SemanticCompletionError {
    fn from(error: automatic_imports::AutomaticImportError) -> Self {
        Self::AutomaticImport(error)
    }
}

#[derive(Clone, Copy)]
struct Candidate {
    entity: SemanticEntity,
    kind: SemanticCompletionKind,
}

impl AnalysisSnapshot {
    /// Enumerates names visible in the checked lexical and module scopes at `offset`.
    ///
    /// Candidate identity and shadowing come from compiler-owned namespaces. Source ranges are
    /// used only to select the containing scope and to exclude sequential local declarations that
    /// occur after the cursor.
    ///
    /// # Errors
    ///
    /// Returns an internal query error when source context or a normalized selection authority is
    /// inconsistent with the retained generation.
    pub fn semantic_completions(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Result<Box<[SemanticCompletion]>, SemanticCompletionError> {
        let keyword_completions = keywords::completions(self, source, offset);
        let Some(program) = self.semantic_authority() else {
            return Ok(keyword_completions);
        };
        let index = program.source_index();
        let module = SourceContext::resolve(index, source)?.module();
        if let Some(checked) = program.checked() {
            if let Some(completions) =
                associated_types::checked_completions(checked, index, source, offset, module)?
            {
                return Ok(completions);
            }
            if let Some(completions) = enum_patterns::checked_completions(
                checked,
                index,
                self.syntax_trees(),
                source,
                offset,
                module,
            )? {
                return Ok(completions);
            }
            if let Some(completions) = structural_fields::checked_completions(
                checked,
                index,
                self.syntax_trees(),
                source,
                offset,
                module,
            )? {
                return Ok(completions);
            }
            if let Some(completions) =
                construction::checked_completions(checked, index, source, offset, module)?
            {
                return Ok(completions);
            }
            if let Some(completions) =
                checked_member_completions(checked, index, source, offset, module)?
            {
                return Ok(completions);
            }
        }
        if let Some(completions) = interrupted_completions(program, source, offset, module)? {
            return Ok(completions);
        }
        let mut candidates = BTreeMap::new();
        add_module_candidates(program.graph(), module, &mut candidates);
        if let Some((body, scope)) = containing_scope(index, source, offset) {
            add_scope_candidates(program, index, source, offset, body, scope, &mut candidates);
        }
        let spellings = VisibleSpellings::for_source(program.graph(), module, index, source);
        let automatic_imports =
            automatic_imports::completions(self, program, source, module, &candidates)?;
        let mut completions = candidates
            .into_iter()
            .filter_map(|(name, candidate)| {
                let label = program.graph().symbols().spelling(name)?;
                Some(SemanticCompletion::new(
                    label,
                    candidate.kind,
                    program.completion_detail(candidate.entity, &spellings),
                ))
            })
            .collect::<Vec<_>>();
        completions.extend(automatic_imports);
        completions.extend(keyword_completions);
        Ok(completions.into_boxed_slice())
    }
}

fn interrupted_completions(
    authority: SemanticAuthority<'_>,
    source: SourceId,
    offset: ByteOffset,
    module: nocter_model::ModuleId,
) -> Result<Option<Box<[SemanticCompletion]>>, SemanticCompletionError> {
    let Some(recovery) = authority.body_analysis() else {
        return Ok(None);
    };
    let Some(interruption) = recovery.interruption_at(source, offset) else {
        return Ok(None);
    };
    let index = recovery.prepared().source_index();
    let spellings =
        VisibleSpellings::for_source(recovery.prepared().graph(), module, index, source);
    match interruption.kind() {
        TypedBodyInterruptionKind::MemberSelection { .. } => {
            let Some(candidates) = recovery.interrupted_member_completions(interruption, source)
            else {
                return Ok(None);
            };
            let candidates = candidates?;
            Ok(Some(
                candidates
                    .iter()
                    .filter_map(|candidate| {
                        let label = recovery
                            .prepared()
                            .graph()
                            .symbols()
                            .spelling(candidate.name())?;
                        let (kind, entity) = completion_target(candidate.target());
                        let detail = entity
                            .and_then(|entity| {
                                prepared_presentation(recovery.prepared(), entity, &spellings)
                            })
                            .map(|presentation| Box::<str>::from(presentation.code()));
                        Some(SemanticCompletion::new(label, kind, detail))
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ))
        }
        TypedBodyInterruptionKind::ConstructionSelection { .. } => {
            let Some(candidates) =
                recovery.interrupted_construction_completions(interruption, source)
            else {
                return Ok(None);
            };
            let candidates = candidates?;
            Ok(Some(construction::render_prepared_completions(
                recovery.prepared(),
                &spellings,
                &candidates,
            )))
        }
        TypedBodyInterruptionKind::StructuralConstruction { .. } => {
            let Some(candidates) =
                recovery.interrupted_structural_field_completions(interruption, source)
            else {
                return Ok(None);
            };
            let candidates = candidates?;
            Ok(Some(structural_fields::render_prepared_completions(
                recovery.prepared(),
                &spellings,
                &candidates,
            )))
        }
        TypedBodyInterruptionKind::EnumPattern { .. } => {
            let Some(candidates) =
                recovery.interrupted_enum_pattern_completions(interruption, source)
            else {
                return Ok(None);
            };
            let candidates = candidates?;
            Ok(Some(enum_patterns::render_prepared_completions(
                recovery.prepared(),
                &spellings,
                &candidates,
            )))
        }
        TypedBodyInterruptionKind::AssociatedTypeProjection { .. } => {
            let Some(candidates) = recovery.interrupted_associated_type_completions(interruption)
            else {
                return Ok(None);
            };
            let candidates = candidates?;
            Ok(Some(associated_types::render_prepared_completions(
                recovery.prepared(),
                &spellings,
                &candidates,
            )?))
        }
        TypedBodyInterruptionKind::OutcomeContract { .. } => Ok(None),
    }
}

fn checked_member_completions(
    program: &CheckedProgram,
    index: &SourceIndex,
    source: SourceId,
    offset: ByteOffset,
    module: nocter_model::ModuleId,
) -> Result<Option<Box<[SemanticCompletion]>>, SemanticCompletionError> {
    let member_range = index
        .bindings_in(source)
        .filter(|binding| {
            binding.role() == SourceRole::Reference
                && matches!(binding.entity(), SemanticEntity::Callable(_))
                && binding.origin().span().range().contains_cursor(offset)
        })
        .map(|binding| binding.origin().span().range())
        .min_by_key(|range| range.len());
    let Some(member_range) = member_range else {
        return Ok(None);
    };
    let receiver_selection = index
        .bindings_in(source)
        .filter_map(|binding| {
            let SemanticEntity::BodyNode(body_id, node_id) = binding.entity() else {
                return None;
            };
            let range = binding.origin().span().range();
            if !range.contains_cursor(offset) || !range.contains_range(member_range) {
                return None;
            }
            let node = program.bodies().get(body_id)?.nodes().get(node_id)?;
            let CheckedOperation::Call(call) = node.operation() else {
                return None;
            };
            Some((body_id, range, call.receiver()?))
        })
        .min_by_key(|(_, range, _)| range.len())
        .map(|(body, _, receiver)| (body, receiver));
    let Some((body_id, receiver)) = receiver_selection else {
        return Ok(None);
    };
    let body = program
        .graph()
        .declarations()
        .bodies()
        .get(body_id)
        .ok_or(MemberCompletionError::MissingBody(body_id))?;
    let receiver_type = program
        .bodies()
        .get(body_id)
        .ok_or(MemberCompletionError::MissingBody(body_id))?
        .nodes()
        .get(receiver.value())
        .ok_or(MemberCompletionError::MissingReceiver(receiver.value()))?
        .ty();
    let (available, owned) = receiver_access(receiver.preparation());
    let candidates = program.member_completions(MemberCompletionContext::new(
        body.owner(),
        source,
        receiver_type,
        available,
        owned,
    ))?;
    let spellings = VisibleSpellings::for_source(program.graph(), module, index, source);
    Ok(Some(
        candidates
            .iter()
            .filter_map(|candidate| {
                let label = program.graph().symbols().spelling(candidate.name())?;
                let (kind, entity) = completion_target(candidate.target());
                let detail = entity
                    .and_then(|entity| presentation(program, entity, &spellings))
                    .map(|presentation| Box::<str>::from(presentation.code()));
                Some(SemanticCompletion::new(label, kind, detail))
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    ))
}

const fn completion_target(
    target: MemberCompletionTarget,
) -> (SemanticCompletionKind, Option<SemanticEntity>) {
    match target {
        MemberCompletionTarget::Field(field) => (
            SemanticCompletionKind::Field,
            Some(SemanticEntity::Field(field)),
        ),
        MemberCompletionTarget::Method { surface } => (
            SemanticCompletionKind::Method,
            match surface {
                Some(surface) => Some(SemanticEntity::Callable(surface)),
                None => None,
            },
        ),
    }
}

const fn receiver_access(preparation: ReceiverPreparation) -> (BorrowCapability, bool) {
    match preparation {
        ReceiverPreparation::Owned => (BorrowCapability::ReadWrite, true),
        ReceiverPreparation::BorrowPlace(capability)
        | ReceiverPreparation::BorrowTemporary(capability)
        | ReceiverPreparation::PreserveBorrow(capability) => (capability, false),
        ReceiverPreparation::WeakenReadwriteBorrow => (BorrowCapability::ReadWrite, false),
    }
}

fn containing_scope(
    index: &SourceIndex,
    source: SourceId,
    offset: ByteOffset,
) -> Option<(BodyId, BodyScopeId)> {
    index
        .bindings_in(source)
        .filter(|binding| binding.origin().span().range().contains_cursor(offset))
        .filter_map(|binding| match binding.entity() {
            SemanticEntity::BodyScope(body, scope) => {
                Some((body, scope, binding.origin().span().range()))
            }
            _ => None,
        })
        .min_by_key(|(_, _, range)| range.len())
        .map(|(body, scope, _)| (body, scope))
}

fn add_module_candidates(
    graph: &DeclarationGraph,
    module: nocter_model::ModuleId,
    candidates: &mut BTreeMap<Symbol, Candidate>,
) {
    let Some(namespace) = graph.module_namespaces().get(module) else {
        return;
    };
    for entry in namespace.fallback() {
        if let Some(candidate) = exported_candidate(graph, entry.target()) {
            candidates.insert(entry.name(), candidate);
        }
    }
    for entry in namespace.authored() {
        if let Some(candidate) = exported_candidate(graph, entry.target()) {
            candidates.insert(entry.name(), candidate);
        }
    }
}

fn add_scope_candidates(
    program: SemanticAuthority<'_>,
    index: &SourceIndex,
    source: SourceId,
    offset: ByteOffset,
    body: BodyId,
    mut scope: BodyScopeId,
    candidates: &mut BTreeMap<Symbol, Candidate>,
) {
    let mut chain = Vec::new();
    loop {
        let Some(current) = program.scope(body, scope) else {
            return;
        };
        chain.push(scope);
        let Some(parent) = current.parent() else {
            break;
        };
        scope = parent;
    }
    for scope in chain.into_iter().rev() {
        let Some(scope) = program.scope(body, scope) else {
            continue;
        };
        for binding in scope.bindings() {
            let Some(candidate) = name_candidate(program.graph(), body, binding.target()) else {
                continue;
            };
            if local_is_available(index, source, offset, candidate.entity) {
                candidates.insert(binding.name(), candidate);
            }
        }
    }
}

fn local_is_available(
    index: &SourceIndex,
    source: SourceId,
    offset: ByteOffset,
    entity: SemanticEntity,
) -> bool {
    let SemanticEntity::LocalBinding(..) = entity else {
        return true;
    };
    index
        .bindings_for(entity)
        .iter()
        .filter(|binding| {
            binding.role() == SourceRole::Declaration && binding.origin().source() == source
        })
        .any(|binding| binding.origin().span().range().end() <= offset)
}

fn name_candidate(graph: &DeclarationGraph, body: BodyId, target: NameTarget) -> Option<Candidate> {
    match target {
        NameTarget::Parameter(parameter) => Some(Candidate {
            entity: SemanticEntity::Parameter(parameter),
            kind: SemanticCompletionKind::Parameter,
        }),
        NameTarget::Local(local) => Some(Candidate {
            entity: SemanticEntity::LocalBinding(body, local),
            kind: SemanticCompletionKind::Variable,
        }),
        NameTarget::Capture(capture) => Some(Candidate {
            entity: SemanticEntity::Capture(body, capture),
            kind: SemanticCompletionKind::Variable,
        }),
        NameTarget::Exported(exported) => exported_candidate(graph, exported),
    }
}

fn exported_candidate(graph: &DeclarationGraph, exported: ExportedEntity) -> Option<Candidate> {
    let declarations = graph.declarations();
    let (entity, kind) = match exported {
        ExportedEntity::BuiltinType(builtin) => (
            SemanticEntity::BuiltinType(builtin),
            SemanticCompletionKind::Type,
        ),
        ExportedEntity::Module(module) => (
            SemanticEntity::Module(module),
            SemanticCompletionKind::Module,
        ),
        ExportedEntity::NominalType(ty) => (
            SemanticEntity::NominalType(ty),
            match declarations.nominal_types().get(ty)?.shape() {
                NominalShape::Struct { .. } => SemanticCompletionKind::Struct,
                NominalShape::Enum { .. } => SemanticCompletionKind::Enum,
            },
        ),
        ExportedEntity::TypeAlias(alias) => (
            SemanticEntity::TypeAlias(alias),
            SemanticCompletionKind::Type,
        ),
        ExportedEntity::Interface(interface) => (
            SemanticEntity::Interface(interface),
            SemanticCompletionKind::Interface,
        ),
        ExportedEntity::Constant(constant) => (
            SemanticEntity::Constant(constant),
            SemanticCompletionKind::Constant,
        ),
        ExportedEntity::Callable(callable) => (
            SemanticEntity::Callable(callable),
            SemanticCompletionKind::Function,
        ),
    };
    Some(Candidate { entity, kind })
}
