use std::collections::BTreeMap;

use nocter_checking::{CheckedProgram, NameTarget};
use nocter_declarations::{ExportedEntity, NominalShape};
use nocter_model::{BodyId, BodyScopeId, Symbol};
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole};

use crate::AnalysisSnapshot;
use crate::presentation::presentation;

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
    Parameter,
    Variable,
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
    #[must_use]
    pub fn semantic_completions(
        &self,
        source: SourceId,
        offset: ByteOffset,
    ) -> Box<[SemanticCompletion]> {
        let Some(target) = self.target() else {
            return Box::new([]);
        };
        let checked = target.program().checked();
        let Some(index) = self.source_index() else {
            return Box::new([]);
        };
        let Some(module) = containing_module(index, source, offset) else {
            return Box::new([]);
        };
        let mut candidates = BTreeMap::new();
        add_module_candidates(checked, module, &mut candidates);
        if let Some((body, scope)) = containing_scope(index, source, offset) {
            add_scope_candidates(checked, index, source, offset, body, scope, &mut candidates);
        }
        candidates
            .into_iter()
            .filter_map(|(name, candidate)| {
                let label = checked.graph().symbols().spelling(name)?;
                let detail = presentation(checked, candidate.entity)
                    .map(|presentation| Box::<str>::from(presentation.code()));
                Some(SemanticCompletion {
                    label: label.into(),
                    kind: candidate.kind,
                    detail,
                })
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}

fn containing_module(
    index: &SourceIndex,
    source: SourceId,
    offset: ByteOffset,
) -> Option<nocter_model::ModuleId> {
    index
        .bindings_in(source)
        .filter(|binding| contains(binding.origin().span().range(), offset))
        .filter_map(|binding| match binding.entity() {
            SemanticEntity::Module(module)
                if matches!(
                    binding.role(),
                    SourceRole::Declaration | SourceRole::Implementation
                ) =>
            {
                Some((module, binding.origin().span().range()))
            }
            _ => None,
        })
        .min_by_key(|(_, range)| range_length(*range))
        .map(|(module, _)| module)
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
    checked: &CheckedProgram,
    module: nocter_model::ModuleId,
    candidates: &mut BTreeMap<Symbol, Candidate>,
) {
    let Some(namespace) = checked.graph().module_namespaces().get(module) else {
        return;
    };
    for entry in namespace.fallback() {
        if let Some(candidate) = exported_candidate(checked, entry.target()) {
            candidates.insert(entry.name(), candidate);
        }
    }
    for entry in namespace.authored() {
        if let Some(candidate) = exported_candidate(checked, entry.target()) {
            candidates.insert(entry.name(), candidate);
        }
    }
}

fn add_scope_candidates(
    checked: &CheckedProgram,
    index: &SourceIndex,
    source: SourceId,
    offset: ByteOffset,
    body: BodyId,
    mut scope: BodyScopeId,
    candidates: &mut BTreeMap<Symbol, Candidate>,
) {
    let Some(checked_body) = checked.bodies().get(body) else {
        return;
    };
    let mut chain = Vec::new();
    loop {
        let Some(current) = checked_body.scopes().get(scope) else {
            return;
        };
        chain.push(scope);
        let Some(parent) = current.parent() else {
            break;
        };
        scope = parent;
    }
    for scope in chain.into_iter().rev() {
        let Some(scope) = checked_body.scopes().get(scope) else {
            continue;
        };
        for binding in scope.bindings() {
            let Some(candidate) = name_candidate(checked, body, binding.target()) else {
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

fn name_candidate(checked: &CheckedProgram, body: BodyId, target: NameTarget) -> Option<Candidate> {
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
        NameTarget::Exported(exported) => exported_candidate(checked, exported),
        NameTarget::Builtin(_) => None,
    }
}

fn exported_candidate(checked: &CheckedProgram, exported: ExportedEntity) -> Option<Candidate> {
    let declarations = checked.graph().declarations();
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

const fn range_length(range: TextRange) -> u32 {
    range.end().get() - range.start().get()
}
