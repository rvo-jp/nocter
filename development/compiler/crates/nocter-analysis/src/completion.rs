use std::collections::BTreeMap;

use nocter_checking::{
    BodyScope, CheckedOperation, CheckedProgram, MemberCompletionContext, MemberCompletionTarget,
    NameTarget, PreparedSemanticProgram, ReceiverPreparation,
};
use nocter_declarations::{DeclarationGraph, ExportedEntity, NominalShape};
use nocter_model::{BodyId, BodyScopeId, BorrowCapability, Symbol};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole};

use crate::AnalysisSnapshot;
use crate::presentation::visible_spelling::VisibleSpellings;
use crate::presentation::{name_recovery_presentation, prepared_presentation, presentation};
use crate::source_context::{SourceContext, SourceContextError};

/// One compiler-selected name visible at an exact source position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCompletion {
    label: Box<str>,
    kind: SemanticCompletionKind,
    detail: Option<Box<str>>,
}

impl SemanticCompletion {
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
    Field,
    Method,
    Parameter,
    Variable,
}

#[derive(Clone, Copy)]
struct Candidate {
    entity: SemanticEntity,
    kind: SemanticCompletionKind,
}

enum CompletionProgram<'a> {
    Checked {
        program: &'a CheckedProgram,
        index: &'a SourceIndex,
    },
    Prepared(&'a PreparedSemanticProgram),
    Names(&'a nocter_checking::NameAnalysisRecovery),
}

impl<'a> CompletionProgram<'a> {
    const fn graph(&self) -> &'a DeclarationGraph {
        match self {
            Self::Checked { program, .. } => program.graph(),
            Self::Prepared(program) => program.graph(),
            Self::Names(program) => program.graph(),
        }
    }

    const fn index(&self) -> &'a SourceIndex {
        match self {
            Self::Checked { index, .. } => index,
            Self::Prepared(program) => program.source_index(),
            Self::Names(program) => program.source_index(),
        }
    }

    fn scope(&self, body: BodyId, scope: BodyScopeId) -> Option<&'a BodyScope> {
        match self {
            Self::Checked { program, .. } => program.bodies().get(body)?.scopes().get(scope),
            Self::Prepared(program) => program.body_names().get(body)?.scopes().get(scope),
            Self::Names(program) => program.body_names().get(body)?.scopes().get(scope),
        }
    }

    fn detail(&self, entity: SemanticEntity, spellings: &VisibleSpellings) -> Option<Box<str>> {
        match self {
            Self::Checked { program, .. } => presentation(program, entity, spellings),
            Self::Prepared(program) => prepared_presentation(program, entity, spellings),
            Self::Names(program) => name_recovery_presentation(program, entity, spellings),
        }
        .map(|presentation| Box::<str>::from(presentation.code()))
    }
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
    /// Returns an internal context error when a reached source has no unique semantic module.
    pub fn semantic_completions(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Result<Box<[SemanticCompletion]>, SourceContextError> {
        let program = if let Some(target) = self.target() {
            CompletionProgram::Checked {
                program: target.program().checked(),
                index: target.source_index(),
            }
        } else if let Some(prepared) = self.prepared_semantics() {
            CompletionProgram::Prepared(prepared)
        } else if let Some(recovery) = self.name_recovery() {
            CompletionProgram::Names(recovery)
        } else {
            return Ok(Box::new([]));
        };
        let index = program.index();
        let module = SourceContext::resolve(index, source)?.module();
        if let CompletionProgram::Checked {
            program: checked, ..
        } = program
            && let Some(completions) =
                checked_member_completions(checked, index, source, offset, module)
        {
            return Ok(completions);
        }
        if let Some(completions) = interrupted_member_completions(self, source, offset, module) {
            return Ok(completions);
        }
        let mut candidates = BTreeMap::new();
        add_module_candidates(program.graph(), module, &mut candidates);
        if let Some((body, scope)) = containing_scope(index, source, offset) {
            add_scope_candidates(
                &program,
                index,
                source,
                offset,
                body,
                scope,
                &mut candidates,
            );
        }
        let spellings = VisibleSpellings::new(program.graph(), module);
        Ok(candidates
            .into_iter()
            .filter_map(|(name, candidate)| {
                let label = program.graph().symbols().spelling(name)?;
                Some(SemanticCompletion {
                    label: label.into(),
                    kind: candidate.kind,
                    detail: program.detail(candidate.entity, &spellings),
                })
            })
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }
}

fn interrupted_member_completions(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    offset: ByteOffset,
    module: nocter_model::ModuleId,
) -> Option<Box<[SemanticCompletion]>> {
    let recovery = snapshot.body_recovery()?;
    let origin = recovery.interruption()?.origin();
    if origin.source() != source || !contains(origin.span().range(), offset) {
        return None;
    }
    let candidates = recovery.interrupted_member_completions(module)?.ok()?;
    let spellings = VisibleSpellings::new(recovery.prepared().graph(), module);
    Some(
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
                Some(SemanticCompletion {
                    label: label.into(),
                    kind,
                    detail,
                })
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn checked_member_completions(
    program: &CheckedProgram,
    index: &SourceIndex,
    source: SourceId,
    offset: ByteOffset,
    module: nocter_model::ModuleId,
) -> Option<Box<[SemanticCompletion]>> {
    let member_range = index
        .bindings_in(source)
        .filter(|binding| {
            binding.role() == SourceRole::Reference
                && matches!(binding.entity(), SemanticEntity::Callable(_))
                && contains(binding.origin().span().range(), offset)
        })
        .map(|binding| binding.origin().span().range())
        .min_by_key(|range| range_length(*range))?;
    let (body_id, receiver) = index
        .bindings_in(source)
        .filter_map(|binding| {
            let SemanticEntity::BodyNode(body_id, node_id) = binding.entity() else {
                return None;
            };
            let range = binding.origin().span().range();
            if !contains(range, offset) || !contains_range(range, member_range) {
                return None;
            }
            let node = program.bodies().get(body_id)?.nodes().get(node_id)?;
            let CheckedOperation::Call(call) = node.operation() else {
                return None;
            };
            Some((body_id, range, call.receiver()?))
        })
        .min_by_key(|(_, range, _)| range_length(*range))
        .map(|(body, _, receiver)| (body, receiver))?;
    let body = program.graph().declarations().bodies().get(body_id)?;
    let receiver_type = program
        .bodies()
        .get(body_id)?
        .nodes()
        .get(receiver.value())?
        .ty();
    let (available, owned) = receiver_access(receiver.preparation());
    let candidates = program
        .member_completions(MemberCompletionContext::new(
            body.owner(),
            module,
            receiver_type,
            available,
            owned,
        ))
        .ok()?;
    let spellings = VisibleSpellings::new(program.graph(), module);
    Some(
        candidates
            .iter()
            .filter_map(|candidate| {
                let label = program.graph().symbols().spelling(candidate.name())?;
                let (kind, entity) = completion_target(candidate.target());
                let detail = entity
                    .and_then(|entity| presentation(program, entity, &spellings))
                    .map(|presentation| Box::<str>::from(presentation.code()));
                Some(SemanticCompletion {
                    label: label.into(),
                    kind,
                    detail,
                })
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

const fn completion_target(
    target: MemberCompletionTarget,
) -> (SemanticCompletionKind, Option<SemanticEntity>) {
    match target {
        MemberCompletionTarget::Field(nocter_model::FieldIdentity::Declared(field)) => (
            SemanticCompletionKind::Field,
            Some(SemanticEntity::Field(field)),
        ),
        MemberCompletionTarget::Field(nocter_model::FieldIdentity::Builtin(field)) => (
            SemanticCompletionKind::Field,
            Some(SemanticEntity::BuiltinField(field)),
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
        .filter(|binding| contains(binding.origin().span().range(), offset))
        .filter_map(|binding| match binding.entity() {
            SemanticEntity::BodyScope(body, scope) => {
                Some((body, scope, binding.origin().span().range()))
            }
            _ => None,
        })
        .min_by_key(|(_, _, range)| range_length(*range))
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
    program: &CompletionProgram<'_>,
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
        NameTarget::Builtin(_) => None,
    }
}

fn exported_candidate(graph: &DeclarationGraph, exported: ExportedEntity) -> Option<Candidate> {
    let declarations = graph.declarations();
    let (entity, kind) = match exported {
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
        ExportedEntity::Callable(callable) => (
            SemanticEntity::Callable(callable),
            SemanticCompletionKind::Function,
        ),
    };
    Some(Candidate { entity, kind })
}

const fn contains(range: TextRange, offset: ByteOffset) -> bool {
    range.start().get() <= offset.get() && offset.get() <= range.end().get()
}

const fn contains_range(outer: TextRange, inner: TextRange) -> bool {
    outer.start().get() <= inner.start().get() && inner.end().get() <= outer.end().get()
}

const fn range_length(range: TextRange) -> u32 {
    range.end().get() - range.start().get()
}
