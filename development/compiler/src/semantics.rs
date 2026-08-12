//! Compiler-owned semantic roles attached to validated trusted declarations.

use crate::semantic::{DefId, SemanticDb};
use crate::source::ByteSpan;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllocatorCapabilityKind {
    Aborting,
    Recoverable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllocationFailurePolicy {
    Abort,
    Recoverable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllocationSource {
    CurrentContext,
    Input(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrustedDeclarationRole {
    AllocatorCapability(AllocatorCapabilityKind),
    CurrentAllocationContext,
    AllocationOperation {
        source: AllocationSource,
        failure_policy: AllocationFailurePolicy,
    },
    AllocationMutation {
        target: usize,
        source: AllocationSource,
        fallback_to_current: bool,
    },
    RegionEnter,
    RegionRelease,
    AllocationAbort,
    IndependentFallibleError,
    StaticResult,
    BorrowedProjection {
        source: usize,
    },
    OwnedValueTransfer {
        source: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeCallable {
    pub(crate) declaration: ByteSpan,
    pub(crate) target_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterpolationRuntime {
    pub(crate) string_type_declaration: ByteSpan,
    pub(crate) constructor: RuntimeCallable,
    pub(crate) format_interface_declaration: ByteSpan,
    pub(crate) format_interface_canonical_name: String,
    pub(crate) format_method_declaration: ByteSpan,
    pub(crate) format_method_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationProtocol {
    pub(crate) interface_declaration: ByteSpan,
    /// Qualified identity derived from the validated declaration's owning module.
    pub(crate) interface_canonical_name: String,
    pub(crate) method_declaration: ByteSpan,
    pub(crate) method_name: String,
    pub(crate) associated_type: Option<IterationAssociatedType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationAssociatedType {
    pub(crate) declaration: ByteSpan,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationRuntime {
    pub(crate) iterator: IterationProtocol,
    pub(crate) exact_size: IterationProtocol,
}

impl InterpolationRuntime {
    pub(crate) fn new(
        string_type_declaration: ByteSpan,
        constructor: RuntimeCallable,
        format_interface_declaration: ByteSpan,
        format_interface_canonical_name: String,
        format_method_declaration: ByteSpan,
        format_method_name: String,
    ) -> Self {
        Self {
            string_type_declaration,
            constructor,
            format_interface_declaration,
            format_interface_canonical_name,
            format_method_declaration,
            format_method_name,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TrustedDeclarationInputs {
    roles: HashMap<ByteSpan, TrustedDeclarationRole>,
    interpolation_runtime: Option<InterpolationRuntime>,
    iteration_runtime: Option<IterationRuntime>,
}

impl TrustedDeclarationInputs {
    pub(crate) fn insert(&mut self, declaration: ByteSpan, role: TrustedDeclarationRole) {
        self.roles.insert(declaration, role);
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.roles.extend(other.roles);
        if other.interpolation_runtime.is_some() {
            self.interpolation_runtime = other.interpolation_runtime;
        }
        if other.iteration_runtime.is_some() {
            self.iteration_runtime = other.iteration_runtime;
        }
    }

    #[cfg(test)]
    pub(crate) fn role(&self, declaration: ByteSpan) -> Option<TrustedDeclarationRole> {
        self.roles.get(&declaration).copied()
    }

    pub(crate) fn bind(&self, semantic_db: Arc<SemanticDb>) -> TrustedDeclarationFacts {
        let roles = self
            .roles
            .iter()
            .map(|(span, role)| {
                let def_id = semantic_db.definition_at(*span).unwrap_or_else(|| {
                    panic!("trusted declaration at {span:?} has no semantic definition")
                });
                (def_id, *role)
            })
            .collect();
        TrustedDeclarationFacts {
            semantic_db,
            roles,
            interpolation_runtime: self.interpolation_runtime.clone(),
            iteration_runtime: self.iteration_runtime.clone(),
        }
    }

    pub(crate) fn set_interpolation_runtime(&mut self, runtime: InterpolationRuntime) {
        self.interpolation_runtime = Some(runtime);
    }

    pub(crate) fn set_iteration_runtime(&mut self, runtime: IterationRuntime) {
        self.iteration_runtime = Some(runtime);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrustedDeclarationFacts {
    semantic_db: Arc<SemanticDb>,
    roles: HashMap<DefId, TrustedDeclarationRole>,
    interpolation_runtime: Option<InterpolationRuntime>,
    iteration_runtime: Option<IterationRuntime>,
}

impl TrustedDeclarationFacts {
    pub(crate) fn new(semantic_db: Arc<SemanticDb>) -> Self {
        Self {
            semantic_db,
            roles: HashMap::new(),
            interpolation_runtime: None,
            iteration_runtime: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn insert(&mut self, declaration: ByteSpan, role: TrustedDeclarationRole) {
        let def_id = self
            .semantic_db
            .definition_at(declaration)
            .unwrap_or_else(|| panic!("trusted declaration at {declaration:?} has no DefId"));
        self.roles.insert(def_id, role);
    }

    pub(crate) fn role(&self, declaration: ByteSpan) -> Option<TrustedDeclarationRole> {
        let def_id = self.semantic_db.definition_at(declaration)?;
        self.roles.get(&def_id).copied()
    }

    #[cfg(test)]
    pub(crate) fn set_interpolation_runtime(&mut self, runtime: InterpolationRuntime) {
        self.interpolation_runtime = Some(runtime);
    }

    pub(crate) fn interpolation_runtime(&self) -> Option<&InterpolationRuntime> {
        self.interpolation_runtime.as_ref()
    }

    pub(crate) fn iteration_runtime(&self) -> Option<&IterationRuntime> {
        self.iteration_runtime.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Item;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    #[test]
    fn bound_roles_follow_definition_identity_instead_of_the_registered_span() {
        let mut sources = SourceMap::new();
        let source = sources.add_source("trusted.nct", None, "struct Arena { state: usize }");
        let lexed = lex(&sources, source);
        let ast = parse(&sources, source, &lexed.tokens).ast.unwrap();
        let Item::Struct(struct_) = &ast.items[0] else {
            panic!("expected struct");
        };
        let mut inputs = TrustedDeclarationInputs::default();
        inputs.insert(
            struct_.span,
            TrustedDeclarationRole::AllocatorCapability(AllocatorCapabilityKind::Aborting),
        );
        let db = Arc::new(SemanticDb::from_files(std::slice::from_ref(&ast)));
        let facts = inputs.bind(db);

        assert_eq!(
            facts.role(struct_.name_span),
            Some(TrustedDeclarationRole::AllocatorCapability(
                AllocatorCapabilityKind::Aborting
            ))
        );
    }
}
