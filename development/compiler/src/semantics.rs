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
pub(crate) struct RuntimeCallableInput {
    pub(crate) declaration: ByteSpan,
    pub(crate) target_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterpolationRuntimeInput {
    pub(crate) string_type_declaration: ByteSpan,
    pub(crate) constructor: RuntimeCallableInput,
    pub(crate) format_interface_declaration: ByteSpan,
    pub(crate) format_interface_canonical_name: String,
    pub(crate) format_method_declaration: ByteSpan,
    pub(crate) format_method_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationProtocol {
    pub(crate) interface_definition: DefId,
    /// Qualified identity derived from the validated declaration's owning module.
    pub(crate) interface_canonical_name: String,
    pub(crate) method_definition: DefId,
    pub(crate) method_name: String,
    pub(crate) associated_type: Option<IterationAssociatedType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationAssociatedType {
    pub(crate) definition: DefId,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationRuntime {
    pub(crate) iterator: IterationProtocol,
    pub(crate) exact_size: IterationProtocol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationProtocolInput {
    pub(crate) interface_declaration: ByteSpan,
    pub(crate) interface_canonical_name: String,
    pub(crate) method_declaration: ByteSpan,
    pub(crate) method_name: String,
    pub(crate) associated_type: Option<IterationAssociatedTypeInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationAssociatedTypeInput {
    pub(crate) declaration: ByteSpan,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IterationRuntimeInput {
    pub(crate) iterator: IterationProtocolInput,
    pub(crate) exact_size: IterationProtocolInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeCallable {
    pub(crate) definition: DefId,
    pub(crate) target_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterpolationRuntime {
    pub(crate) string_type_definition: DefId,
    pub(crate) constructor: RuntimeCallable,
    pub(crate) format_interface_definition: DefId,
    pub(crate) format_interface_canonical_name: String,
    pub(crate) format_method_definition: DefId,
    pub(crate) format_method_name: String,
}

impl InterpolationRuntimeInput {
    pub(crate) fn new(
        string_type_declaration: ByteSpan,
        constructor: RuntimeCallableInput,
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

    fn bind(&self, semantic_db: &SemanticDb) -> InterpolationRuntime {
        InterpolationRuntime {
            string_type_definition: required_definition(
                semantic_db,
                self.string_type_declaration,
                "trusted String type",
            ),
            constructor: RuntimeCallable {
                definition: required_definition(
                    semantic_db,
                    self.constructor.declaration,
                    "trusted interpolation constructor",
                ),
                target_name: self.constructor.target_name.clone(),
            },
            format_interface_definition: required_definition(
                semantic_db,
                self.format_interface_declaration,
                "trusted Format interface",
            ),
            format_interface_canonical_name: self.format_interface_canonical_name.clone(),
            format_method_definition: required_definition(
                semantic_db,
                self.format_method_declaration,
                "trusted formatting method",
            ),
            format_method_name: self.format_method_name.clone(),
        }
    }
}

impl IterationRuntimeInput {
    fn bind(&self, semantic_db: &SemanticDb) -> IterationRuntime {
        IterationRuntime {
            iterator: self.iterator.bind(semantic_db),
            exact_size: self.exact_size.bind(semantic_db),
        }
    }
}

impl IterationProtocolInput {
    fn bind(&self, semantic_db: &SemanticDb) -> IterationProtocol {
        IterationProtocol {
            interface_definition: required_definition(
                semantic_db,
                self.interface_declaration,
                "trusted iteration interface",
            ),
            interface_canonical_name: self.interface_canonical_name.clone(),
            method_definition: required_definition(
                semantic_db,
                self.method_declaration,
                "trusted iteration method",
            ),
            method_name: self.method_name.clone(),
            associated_type: self.associated_type.as_ref().map(|associated| {
                IterationAssociatedType {
                    definition: required_definition(
                        semantic_db,
                        associated.declaration,
                        "trusted iteration associated type",
                    ),
                    name: associated.name.clone(),
                }
            }),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TrustedDeclarationInputs {
    roles: HashMap<ByteSpan, TrustedDeclarationRole>,
    interpolation_runtime: Option<InterpolationRuntimeInput>,
    iteration_runtime: Option<IterationRuntimeInput>,
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
        let interpolation_runtime = self
            .interpolation_runtime
            .as_ref()
            .map(|runtime| runtime.bind(&semantic_db));
        let iteration_runtime = self
            .iteration_runtime
            .as_ref()
            .map(|runtime| runtime.bind(&semantic_db));
        TrustedDeclarationFacts {
            semantic_db,
            roles,
            interpolation_runtime,
            iteration_runtime,
        }
    }

    pub(crate) fn set_interpolation_runtime(&mut self, runtime: InterpolationRuntimeInput) {
        self.interpolation_runtime = Some(runtime);
    }

    pub(crate) fn set_iteration_runtime(&mut self, runtime: IterationRuntimeInput) {
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
        self.role_definition(def_id)
    }

    pub(crate) fn role_definition(&self, definition: DefId) -> Option<TrustedDeclarationRole> {
        self.roles.get(&definition).copied()
    }

    #[cfg(test)]
    pub(crate) fn set_interpolation_runtime(&mut self, runtime: InterpolationRuntimeInput) {
        self.interpolation_runtime = Some(runtime.bind(&self.semantic_db));
    }

    pub(crate) fn interpolation_runtime(&self) -> Option<&InterpolationRuntime> {
        self.interpolation_runtime.as_ref()
    }

    pub(crate) fn iteration_runtime(&self) -> Option<&IterationRuntime> {
        self.iteration_runtime.as_ref()
    }
}

fn required_definition(semantic_db: &SemanticDb, span: ByteSpan, role: &str) -> DefId {
    semantic_db
        .definition_at(span)
        .unwrap_or_else(|| panic!("{role} at {span:?} has no semantic definition"))
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
