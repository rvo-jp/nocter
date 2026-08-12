use crate::abi::{
    AbiType, AbiValue, abi_value_from_type_expr, abi_value_from_type_expr_with_resolver,
};
use crate::analysis::{
    CompileUnitAnalysis, FileAnalysis,
    call_specializations::{collect_call_specializations, method_owner_substitutions_for_self_ty},
};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, BindingStmt, Block, CallExpr, DestructDecl, Expr,
    ForRangeStmt, FunctionDecl, IdentifierExpr, Item, MemberExpr, MethodDecl, MethodOwnerDecl,
    OtherwiseExpr, Parameter, PayloadEnumPatternTargetShape, Stmt, StructLiteralField,
    SwitchPayloadPattern, TypeExpr, TypeReference, UnaryOperator, canonical_type_expr,
    substitute_type_expr_parameters,
};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::integer::IntegerType;
use crate::ir::CallTarget;
use crate::literals::decode_integer_literal_value;
use crate::outcomes::outcome_shape_with_resolver;
use crate::resolve::{ResolveOutput, SymbolKind, TypeSymbol, TypeSymbolKind};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{
    FunctionCallSpecialization, MethodCallSpecialization, TypecheckFacts,
    TypecheckMethodReceiverKind, TypecheckPayloadBindingMode, TypecheckScalarViewKind,
    TypecheckSliceElementKind,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::test_entry::TestDeclarationId;

pub(super) fn v0_buildability_diagnostics(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
) -> Vec<Diagnostic> {
    let Some(root) = analysis.root_file() else {
        return Vec::new();
    };

    let root_source = root.ast.span.source;
    let nocter_home = analysis.nocter_home.as_deref();
    let index = CallableIndex::new(analysis, root_source);
    let mut queue = VecDeque::from([CallTarget::same_file(DEFAULT_ENTRY_NAME)]);
    let mut seen = HashSet::new();
    let mut diagnostics = Vec::new();

    while let Some(target) = queue.pop_front() {
        if !seen.insert(target.clone()) {
            continue;
        }
        let Some(callable) = index.definition(&target) else {
            continue;
        };
        collect_callable_diagnostics(
            callable,
            sources,
            root_source,
            &index.names,
            &index.resolved_sources,
            nocter_home,
            &mut queue,
            &mut diagnostics,
        );
    }

    diagnostics
}

pub(super) fn v0_test_buildability_diagnostics(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    test: &TestDeclarationId,
) -> Vec<Diagnostic> {
    let Some(root) = analysis.root_file() else {
        return Vec::new();
    };
    let Some(test_decl) = test.resolve(&root.ast) else {
        return vec![
            Diagnostic::error(
                "E0700",
                format!(
                    "cannot validate missing test `{}` for native build",
                    test.name()
                ),
            )
            .with_primary_span_if_absent(sources, root.ast.span),
        ];
    };

    let root_source = root.ast.span.source;
    let nocter_home = analysis.nocter_home.as_deref();
    let index = CallableIndex::new(analysis, root_source);
    let callable = IndexedCallable::new_test(test_decl, root);
    let mut queue = VecDeque::new();
    let mut diagnostics = Vec::new();
    collect_callable_diagnostics(
        &callable,
        sources,
        root_source,
        &index.names,
        &index.resolved_sources,
        nocter_home,
        &mut queue,
        &mut diagnostics,
    );
    let mut seen = HashSet::new();
    while let Some(target) = queue.pop_front() {
        if !seen.insert(target.clone()) {
            continue;
        }
        let Some(callable) = index.definition(&target) else {
            continue;
        };
        collect_callable_diagnostics(
            callable,
            sources,
            root_source,
            &index.names,
            &index.resolved_sources,
            nocter_home,
            &mut queue,
            &mut diagnostics,
        );
    }
    diagnostics
}

struct CallableIndex<'a> {
    definitions: HashMap<CallTarget, IndexedCallable<'a>>,
    names: HashMap<ByteSpan, String>,
    resolved_sources: ResolvedSources<'a>,
    root_source: SourceId,
}

type ResolvedSources<'a> = HashMap<SourceId, &'a ResolveOutput>;

impl<'a> CallableIndex<'a> {
    fn new(analysis: &'a CompileUnitAnalysis, root_source: SourceId) -> Self {
        let mut definitions = HashMap::new();
        let mut names = HashMap::new();
        let call_specializations = collect_call_specializations(analysis);
        let resolved_sources = analysis
            .files
            .iter()
            .map(|file| (file.ast.span.source, &file.resolved))
            .collect::<ResolvedSources<'_>>();

        for file in &analysis.files {
            for item in &file.ast.items {
                match item {
                    Item::Function(function)
                        if function.body.is_some() && function.generics.parameters.is_empty() =>
                    {
                        let identity = if function.owner.is_some() {
                            function.member_name_span
                        } else {
                            function.name_span
                        };
                        let declaration = analysis.callable_bodies.canonical_identity(identity);
                        let target = call_target_for_source(
                            declaration.source,
                            root_source,
                            function.name.clone(),
                        );
                        names.insert(declaration, function.name.clone());
                        definitions.insert(
                            target,
                            IndexedCallable::new_function(
                                function,
                                file,
                                &resolved_sources,
                                root_source,
                            ),
                        );
                    }
                    Item::Function(function) if function.body.is_some() => {
                        let member_identity = analysis
                            .callable_bodies
                            .canonical_identity(function.member_name_span);
                        let definition = analysis
                            .semantic_db
                            .definition_at(member_identity)
                            .expect("indexed function must have a semantic definition");
                        for specialization in call_specializations
                            .functions
                            .get(&definition)
                            .into_iter()
                            .flatten()
                        {
                            let target = call_target_for_source(
                                member_identity.source,
                                root_source,
                                specialization.target_name.clone(),
                            );
                            definitions.insert(
                                target,
                                IndexedCallable::new_function_specialization(
                                    function,
                                    specialization.substitutions.clone(),
                                    file,
                                    &resolved_sources,
                                    root_source,
                                ),
                            );
                        }
                    }
                    Item::Function(_) => {}
                    Item::Instance(_) | Item::Conformance(_) => {
                        let owner = item.method_owner().expect("matched method owner");
                        let Some(type_name) = declaration_target_type_name(owner.target_ty())
                        else {
                            continue;
                        };
                        for method in owner.methods() {
                            if method.body.is_some() && owner.generics().parameters.is_empty() {
                                let Some(body) = method.body.as_ref() else {
                                    continue;
                                };
                                let declaration = analysis
                                    .callable_bodies
                                    .canonical_identity(method.name_span);
                                let declaration_source = if declaration == method.name_span {
                                    file.ast.span.source
                                } else {
                                    declaration.source
                                };
                                let name = method_target_name(type_name, &method.name);
                                let target = call_target_for_source(
                                    declaration_source,
                                    root_source,
                                    name.clone(),
                                );
                                names.insert(declaration, name.clone());
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_method(
                                        method,
                                        body,
                                        owner.target_ty(),
                                        HashMap::new(),
                                        file,
                                        &resolved_sources,
                                    ),
                                );
                            } else if method.body.is_some() {
                                let Some(body) = method.body.as_ref() else {
                                    continue;
                                };
                                let declaration = analysis
                                    .callable_bodies
                                    .canonical_identity(method.name_span);
                                let declaration_source = if declaration == method.name_span {
                                    file.ast.span.source
                                } else {
                                    declaration.source
                                };
                                let def_id = analysis
                                    .semantic_db
                                    .definition_at(declaration)
                                    .expect("buildable method must have a semantic definition");
                                for specialization in call_specializations
                                    .methods
                                    .get(&def_id)
                                    .into_iter()
                                    .flatten()
                                {
                                    let substitutions = method_specialization_context_substitutions(
                                        owner,
                                        specialization,
                                        &file.resolved,
                                        &resolved_sources,
                                    );
                                    let target = call_target_for_source(
                                        declaration_source,
                                        root_source,
                                        specialization.target_name.clone(),
                                    );
                                    definitions.insert(
                                        target,
                                        IndexedCallable::new_method(
                                            method,
                                            body,
                                            owner.target_ty(),
                                            substitutions,
                                            file,
                                            &resolved_sources,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    Item::Destruct(destruct) => {
                        if destruct.generics.parameters.is_empty() {
                            let name = drop_target_name(&destruct.target_ty);
                            let target = call_target_for_source(
                                file.ast.span.source,
                                root_source,
                                name.clone(),
                            );
                            names.insert(destruct.keyword_span, name.clone());
                            definitions.insert(
                                target,
                                IndexedCallable::new_drop(
                                    destruct,
                                    &destruct.target_ty,
                                    HashMap::new(),
                                    file,
                                    &resolved_sources,
                                ),
                            );
                        } else {
                            let definition = analysis
                                .semantic_db
                                .definition_at(destruct.keyword_span)
                                .expect("indexed destructor must have a semantic definition");
                            for specialization in call_specializations
                                .drops
                                .get(&definition)
                                .into_iter()
                                .flatten()
                            {
                                let target = call_target_for_source(
                                    file.ast.span.source,
                                    root_source,
                                    specialization.target_name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_drop(
                                        destruct,
                                        &destruct.target_ty,
                                        specialization.substitutions.clone(),
                                        file,
                                        &resolved_sources,
                                    ),
                                );
                            }
                        }
                    }
                    Item::Interface(interface) => {
                        for method in &interface.methods {
                            let Some(body) = &method.body else {
                                continue;
                            };
                            let def_id = analysis
                                .semantic_db
                                .definition_at(method.name_span)
                                .expect("interface method must have a semantic definition");
                            for specialization in call_specializations
                                .methods
                                .get(&def_id)
                                .into_iter()
                                .flatten()
                            {
                                let target = call_target_for_source(
                                    file.ast.span.source,
                                    root_source,
                                    specialization.target_name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_method(
                                        method,
                                        body,
                                        &specialization.self_ty,
                                        specialization.substitutions.clone(),
                                        file,
                                        &resolved_sources,
                                    ),
                                );
                            }
                        }
                    }
                    Item::Construct(construct) => {
                        for (_, function) in construct.functions() {
                            if function.body.is_none() {
                                continue;
                            }
                            let declaration = analysis
                                .callable_bodies
                                .canonical_identity(function.member_name_span);
                            if function.generics.parameters.is_empty() {
                                let target = call_target_for_source(
                                    declaration.source,
                                    root_source,
                                    function.name.clone(),
                                );
                                names.insert(declaration, function.name.clone());
                                let substitutions =
                                    HashMap::from([("Self".to_string(), construct.target.clone())]);
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_function_specialization(
                                        function,
                                        substitutions,
                                        file,
                                        &resolved_sources,
                                        root_source,
                                    ),
                                );
                                continue;
                            }
                            let definition = analysis
                                .semantic_db
                                .definition_at(declaration)
                                .expect("indexed constructor must have a semantic definition");
                            for specialization in call_specializations
                                .functions
                                .get(&definition)
                                .into_iter()
                                .flatten()
                            {
                                let mut substitutions = specialization.substitutions.clone();
                                let self_ty = substitute_type_expr_parameters(
                                    &construct.target,
                                    &substitutions,
                                );
                                substitutions.insert("Self".to_string(), self_ty);
                                let target = call_target_for_source(
                                    declaration.source,
                                    root_source,
                                    specialization.target_name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_function_specialization(
                                        function,
                                        substitutions,
                                        file,
                                        &resolved_sources,
                                        root_source,
                                    ),
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        for (body_id, specializations) in &call_specializations.callables {
            let closure_span = analysis
                .semantic_db
                .body_anchor(*body_id)
                .expect("specialized closure must have an authored body");
            let Some(file) = analysis.file_by_source(closure_span.source) else {
                continue;
            };
            let Some(expression) = crate::ast::closure_expression_by_span(&file.ast, closure_span)
            else {
                continue;
            };
            for specialization in specializations {
                if !matches!(specialization.callable_ty, TypeExpr::Closure(_)) {
                    continue;
                }
                let Some(plan) = file.typecheck_facts.closure_plan(closure_span).cloned() else {
                    continue;
                };
                let receiver_mode = specialization.receiver_mode();
                let target = call_target_for_source(
                    closure_span.source,
                    root_source,
                    specialization.target_name.clone(),
                );
                definitions.insert(
                    target,
                    IndexedCallable::new_closure(
                        expression,
                        plan,
                        receiver_mode,
                        file,
                        &resolved_sources,
                    ),
                );
            }
        }

        Self {
            definitions,
            names,
            resolved_sources,
            root_source,
        }
    }

    fn definition(&self, target: &CallTarget) -> Option<&IndexedCallable<'a>> {
        self.definitions.get(target).or_else(|| {
            let (target_source, target_name) = match target {
                CallTarget::SameFile(name) => (self.root_source, name),
                CallTarget::Imported { source, name } => (*source, name),
            };
            self.definitions.iter().find_map(|(candidate, callable)| {
                let CallTarget::Imported { source, name } = candidate else {
                    return None;
                };
                (name == target_name
                    && self
                        .resolved_sources
                        .get(source)
                        .is_some_and(|resolved| resolved.module_source(*source) == target_source))
                .then_some(callable)
            })
        })
    }
}

struct IndexedCallable<'a> {
    span: ByteSpan,
    body: &'a Block,
    return_type: Option<TypeExpr>,
    substitutions: HashMap<String, TypeExpr>,
    resolved: &'a ResolveOutput,
    typecheck_facts: &'a TypecheckFacts,
    issues: Vec<BuildabilityIssue>,
}

struct BuildabilityIssue {
    span: ByteSpan,
    construct: &'static str,
    help: &'static str,
}

impl<'a> IndexedCallable<'a> {
    fn new_test(test: &'a crate::ast::TestDecl, file: &'a FileAnalysis) -> Self {
        Self {
            span: test.span,
            body: &test.body,
            return_type: Some(test.return_type()),
            substitutions: HashMap::new(),
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues: Vec::new(),
        }
    }

    fn new_function(
        function: &'a FunctionDecl,
        file: &'a FileAnalysis,
        resolved_sources: &ResolvedSources<'a>,
        root_source: SourceId,
    ) -> Self {
        let mut issues = Vec::new();
        issues.extend(callable_function_signature_issues(
            function,
            &HashMap::new(),
            &file.resolved,
            resolved_sources,
            root_source,
        ));
        issues.extend(unsupported_outcome_return_issue(
            function,
            &HashMap::new(),
            &file.resolved,
            resolved_sources,
        ));

        Self {
            span: function.span,
            body: function
                .body
                .as_ref()
                .expect("buildability indexes only body-bearing functions"),
            return_type: Some(function.return_type.clone()),
            substitutions: HashMap::new(),
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }

    fn new_function_specialization(
        function: &'a FunctionDecl,
        mut substitutions: HashMap<String, TypeExpr>,
        file: &'a FileAnalysis,
        resolved_sources: &ResolvedSources<'a>,
        root_source: SourceId,
    ) -> Self {
        crate::typecheck::extend_associated_type_substitutions_with_resolver(
            &mut substitutions,
            &file.resolved,
            |source| resolved_sources.get(&source).copied(),
        );
        let mut issues = Vec::new();
        issues.extend(callable_function_signature_issues(
            function,
            &substitutions,
            &file.resolved,
            resolved_sources,
            root_source,
        ));
        issues.extend(unsupported_outcome_return_issue(
            function,
            &substitutions,
            &file.resolved,
            resolved_sources,
        ));
        let return_type = substitute_type_expr_parameters(&function.return_type, &substitutions);

        Self {
            span: function.span,
            body: function
                .body
                .as_ref()
                .expect("buildability indexes only body-bearing functions"),
            return_type: Some(return_type),
            substitutions,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }

    fn new_method(
        method: &'a MethodDecl,
        body: &'a Block,
        self_ty: &TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        file: &'a FileAnalysis,
        resolved_sources: &ResolvedSources<'a>,
    ) -> Self {
        let mut contextual_substitutions = method_contextual_substitutions(self_ty, &substitutions);
        crate::typecheck::extend_associated_type_substitutions_with_resolver(
            &mut contextual_substitutions,
            &file.resolved,
            |source| resolved_sources.get(&source).copied(),
        );
        let mut issues = Vec::new();
        issues.extend(callable_method_signature_issues(
            method,
            &contextual_substitutions,
            &file.resolved,
            resolved_sources,
        ));
        let return_type =
            substitute_type_expr_parameters(&method.return_type, &contextual_substitutions);
        issues.extend(unsupported_outcome_return_type_issue(
            &return_type,
            method.return_type.span(),
            &file.resolved,
            resolved_sources,
        ));

        Self {
            span: method.span,
            body,
            return_type: Some(return_type),
            substitutions: contextual_substitutions,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }

    fn new_drop(
        drop_: &'a DestructDecl,
        self_ty: &TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        file: &'a FileAnalysis,
        resolved_sources: &ResolvedSources<'a>,
    ) -> Self {
        let mut contextual_substitutions = method_contextual_substitutions(self_ty, &substitutions);
        crate::typecheck::extend_associated_type_substitutions_with_resolver(
            &mut contextual_substitutions,
            &file.resolved,
            |source| resolved_sources.get(&source).copied(),
        );
        let mut issues = Vec::new();
        issues.extend(callable_parameter_issues(
            std::slice::from_ref(&drop_.binding),
            &contextual_substitutions,
            &file.resolved,
            resolved_sources,
        ));

        Self {
            span: drop_.span,
            body: &drop_.body,
            return_type: None,
            substitutions: contextual_substitutions,
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }

    fn new_closure(
        expression: &'a crate::ast::ClosureExpr,
        plan: crate::typecheck::TypecheckClosurePlan,
        receiver_mode: crate::ast::MethodReceiverMode,
        file: &'a FileAnalysis,
        resolved_sources: &ResolvedSources<'a>,
    ) -> Self {
        let issues = closures::closure_signature_issues(
            expression,
            &plan,
            receiver_mode,
            &file.resolved,
            resolved_sources,
        );
        Self {
            span: expression.span,
            body: &expression.body,
            return_type: Some((*plan.ty.return_type).clone()),
            substitutions: HashMap::new(),
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
            issues,
        }
    }
}

mod closures;
mod diagnostics;
mod fixed_arrays;
mod runtime_support;
mod signatures;
mod statements;
mod traversal;
mod types;
mod variants;

use diagnostics::*;
use fixed_arrays::*;
use runtime_support::*;
use signatures::*;
use statements::*;
use traversal::*;
use types::*;
use variants::*;

#[cfg(test)]
mod tests;
