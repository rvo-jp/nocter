use crate::abi::{AbiType, AbiValue, abi_value_from_type_expr_with_resolver};
use crate::analysis::{
    CompileUnitAnalysis, FileAnalysis,
    call_specializations::{collect_call_specializations, method_owner_substitutions_for_self_ty},
};
use crate::ast::{
    Block, CallableDecl, DestructDecl, FunctionDecl, Item, MethodOwnerDecl, Parameter, TypeExpr,
    TypeReference, canonical_type_expr, substitute_type_expr_parameters,
};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::ir::{CallTarget, coercion_symbol_name};
use crate::outcomes::outcome_shape_with_resolver;
use crate::resolve::{ResolveOutput, TypeSymbol, TypeSymbolKind};
use crate::semantic::DefId;
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::{MethodCallSpecialization, TypedHir};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::test_entry::TestDeclarationId;

pub(super) fn native_buildability_diagnostics(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
) -> Vec<Diagnostic> {
    let Some(root) = analysis.root_file() else {
        return Vec::new();
    };

    let root_source = root.ast.span.source;
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
            &analysis.mir_bodies,
            root_source,
            &index.names,
            &index.resolved_sources,
            &mut queue,
            &mut diagnostics,
        );
    }

    diagnostics
}

pub(super) fn native_test_buildability_diagnostics(
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
    let index = CallableIndex::new(analysis, root_source);
    let callable = IndexedCallable::new_test(test_decl, root);
    let mut queue = VecDeque::new();
    let mut diagnostics = Vec::new();
    collect_callable_diagnostics(
        &callable,
        sources,
        &analysis.mir_bodies,
        root_source,
        &index.names,
        &index.resolved_sources,
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
            &analysis.mir_bodies,
            root_source,
            &index.names,
            &index.resolved_sources,
            &mut queue,
            &mut diagnostics,
        );
    }
    diagnostics
}

struct CallableIndex<'a> {
    definitions: HashMap<CallTarget, IndexedCallable<'a>>,
    names: CallableNames,
    resolved_sources: ResolvedSources<'a>,
    root_source: SourceId,
}

type ResolvedSources<'a> = crate::resolve::ResolvedSources<'a>;
#[derive(Default)]
struct CallableNames {
    definitions: HashMap<DefId, String>,
    instances: crate::mir::MonoItemRegistry<String>,
}

impl CallableNames {
    fn insert(&mut self, definition: DefId, name: String) {
        self.definitions.insert(definition, name);
    }

    fn insert_instance(&mut self, instance: crate::mir::CallInstanceKey, name: String) {
        self.instances.insert(instance, name);
    }

    fn get(&self, definition: &DefId) -> Option<&String> {
        self.definitions.get(definition)
    }

    fn get_instance(
        &self,
        instance: &crate::mir::CallInstance,
        typed_hir: &TypedHir,
    ) -> Option<&String> {
        if instance.receiver.is_none()
            && instance.type_arguments.is_empty()
            && let crate::mir::CallableIdentity::Definition(definition) = &instance.callable
        {
            return self.get(definition);
        }
        let key = crate::mir::CallInstanceKey::from_instance(instance, typed_hir);
        self.instances.value_for(key.as_ref()?)
    }
}

fn index_typed_hir_callable_names(
    names: &mut CallableNames,
    typed_hir: &TypedHir,
    analysis: &CompileUnitAnalysis,
) {
    let canonical = |definition| analysis.callable_bodies.canonical_definition(definition);

    for (_, fact) in typed_hir.callable_call_entries() {
        names.insert_instance(
            crate::mir::CallInstanceKey::from_callable_type(
                &fact.specialization.callable_ty,
                fact.specialization.capability,
            ),
            fact.specialization.target_name.clone(),
        );
    }
    for specialization in typed_hir.method_call_specializations() {
        let specialization = crate::typecheck::specialize_method_dispatch_across_resolvers(
            specialization.clone(),
            analysis.files.iter().map(|file| &file.resolved),
        );
        let Some(arguments) = specialization.ordered_type_arguments() else {
            continue;
        };
        names.insert_instance(
            crate::mir::CallInstanceKey::from_types(
                canonical(specialization.def_id),
                Some(&specialization.self_ty),
                arguments,
            ),
            specialization.target_name.clone(),
        );
    }
    for plan in typed_hir.comparison_plans() {
        let plan = if plan.method.is_some() {
            Some(plan.clone())
        } else {
            analysis.files.iter().find_map(|file| {
                crate::typecheck::specialize_comparison_plan(plan.clone(), &file.resolved)
            })
        };
        if let Some(method) = plan.and_then(|plan| plan.method) {
            index_protocol_method_name(names, &method, analysis);
        }
    }
    for plan in typed_hir.index_plans() {
        if let Some(method) = &plan.method {
            index_protocol_method_name(names, method, analysis);
        }
    }
    for (_, plan) in typed_hir.interpolation_plans() {
        names.insert(
            canonical(plan.constructor.definition),
            plan.constructor.target_name.clone(),
        );
        for part in &plan.parts {
            index_protocol_method_name(names, &part.formatter, analysis);
        }
    }
    for (_, plan) in typed_hir.collection_for_plans() {
        let plan = analysis
            .files
            .iter()
            .find_map(|file| {
                crate::typecheck::specialize_collection_plan(plan.clone(), &file.resolved)
            })
            .unwrap_or_else(|| plan.clone());
        for method in plan.conversion.iter().chain(std::iter::once(&plan.step)) {
            index_protocol_method_name(names, method, analysis);
        }
    }
    for (_, plan) in typed_hir.sequence_spread_plans() {
        let plan = analysis
            .files
            .iter()
            .find_map(|file| {
                crate::typecheck::specialize_sequence_spread_plan(plan.clone(), &file.resolved)
            })
            .unwrap_or_else(|| plan.clone());
        for method in plan.conversion.iter().chain([&plan.exact_size, &plan.step]) {
            index_protocol_method_name(names, method, analysis);
        }
    }
    for (_, plan) in typed_hir.coercion_plans() {
        let Some(definition) = plan.def_id else {
            continue;
        };
        names.insert_instance(
            crate::mir::CallInstanceKey::from_types(
                canonical(definition),
                Some(&plan.self_ty),
                std::iter::empty(),
            ),
            crate::ir::coercion_symbol_name(plan),
        );
    }
}

fn index_protocol_method_name(
    names: &mut CallableNames,
    method: &crate::typecheck::TypecheckProtocolMethod,
    analysis: &CompileUnitAnalysis,
) {
    let method = crate::typecheck::specialize_protocol_method_dispatch_across_resolvers(
        method.clone(),
        analysis.files.iter().map(|file| &file.resolved),
    );
    names.insert_instance(
        crate::mir::CallInstanceKey::from_types(
            analysis.callable_bodies.canonical_definition(method.def_id),
            Some(&method.self_ty),
            std::iter::empty(),
        ),
        method.target_name.clone(),
    );
}

impl<'a> CallableIndex<'a> {
    fn new(analysis: &'a CompileUnitAnalysis, root_source: SourceId) -> Self {
        let mut definitions = HashMap::new();
        let mut names = CallableNames::default();
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
                        let (definition, declaration) =
                            canonical_callable_definition(analysis, identity);
                        let target = call_target_for_source(
                            declaration.source,
                            root_source,
                            function.name.clone(),
                        );
                        names.insert(definition, function.name.clone());
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
                        let (definition, member_identity) =
                            canonical_callable_definition(analysis, function.member_name_span);
                        for specialization in call_specializations
                            .functions
                            .get(&definition)
                            .into_iter()
                            .flatten()
                        {
                            if let Some(arguments) = specialization.ordered_type_arguments() {
                                names.insert_instance(
                                    crate::mir::CallInstanceKey::from_types(
                                        definition, None, arguments,
                                    ),
                                    specialization.target_name.clone(),
                                );
                            }
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
                    Item::Instance(instance) => {
                        let owner = item.method_owner().expect("matched method owner");
                        let Some(type_name) = declaration_target_type_name(owner.target_ty())
                        else {
                            continue;
                        };
                        let callables = instance
                            .named_methods()
                            .map(|method| {
                                (&method.callable, method.name_span, method.name.as_str())
                            })
                            .chain(instance.operators().map(|operator| {
                                (
                                    operator.callable(),
                                    operator.anchor_span(),
                                    crate::semantic::OperatorCallableKind::from_declaration(
                                        operator,
                                    )
                                    .lookup_name(),
                                )
                            }));
                        for (method, anchor, method_name) in callables {
                            if method.body.is_some() && owner.generics().parameters.is_empty() {
                                let Some(body) = method.body.as_ref() else {
                                    continue;
                                };
                                let (definition, declaration) =
                                    canonical_callable_definition(analysis, anchor);
                                let declaration_source = declaration.source;
                                let name = method_target_name(type_name, method_name);
                                let target = call_target_for_source(
                                    declaration_source,
                                    root_source,
                                    name.clone(),
                                );
                                names.insert(definition, name.clone());
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
                                let (def_id, declaration) =
                                    canonical_callable_definition(analysis, anchor);
                                let declaration_source = declaration.source;
                                for specialization in call_specializations
                                    .methods
                                    .get(&def_id)
                                    .into_iter()
                                    .flatten()
                                {
                                    if let Some(arguments) = specialization.ordered_type_arguments()
                                    {
                                        names.insert_instance(
                                            crate::mir::CallInstanceKey::from_types(
                                                def_id,
                                                Some(&specialization.self_ty),
                                                arguments,
                                            ),
                                            specialization.target_name.clone(),
                                        );
                                    }
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
                        for entry in instance.coercions() {
                            let callable = entry.callable();
                            if callable.body.is_none() {
                                continue;
                            }
                            let (definition, declaration) =
                                canonical_callable_definition(analysis, entry.as_span);
                            for plan in call_specializations
                                .coercions
                                .get(&definition)
                                .into_iter()
                                .flatten()
                            {
                                let name = coercion_symbol_name(plan);
                                if let Some(definition) = plan.def_id {
                                    names.insert_instance(
                                        crate::mir::CallInstanceKey::from_types(
                                            analysis
                                                .callable_bodies
                                                .canonical_definition(definition),
                                            Some(&plan.self_ty),
                                            std::iter::empty(),
                                        ),
                                        name.clone(),
                                    );
                                }
                                let target = call_target_for_source(
                                    declaration.source,
                                    root_source,
                                    name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_method(
                                        callable,
                                        callable
                                            .body
                                            .as_ref()
                                            .expect("body-bearing coercion was checked"),
                                        &plan.self_ty,
                                        plan.substitutions.clone(),
                                        file,
                                        &resolved_sources,
                                    ),
                                );
                            }
                        }
                    }
                    Item::Conformance(_) => {
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
                                let (definition, declaration) =
                                    canonical_callable_definition(analysis, method.name_span);
                                let name = method_target_name(type_name, &method.name);
                                let target = call_target_for_source(
                                    declaration.source,
                                    root_source,
                                    name.clone(),
                                );
                                names.insert(definition, name);
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_method(
                                        &method.callable,
                                        body,
                                        owner.target_ty(),
                                        HashMap::new(),
                                        file,
                                        &resolved_sources,
                                    ),
                                );
                            } else if let Some(body) = method.body.as_ref() {
                                let (definition, declaration) =
                                    canonical_callable_definition(analysis, method.name_span);
                                for specialization in call_specializations
                                    .methods
                                    .get(&definition)
                                    .into_iter()
                                    .flatten()
                                {
                                    if let Some(arguments) = specialization.ordered_type_arguments()
                                    {
                                        names.insert_instance(
                                            crate::mir::CallInstanceKey::from_types(
                                                definition,
                                                Some(&specialization.self_ty),
                                                arguments,
                                            ),
                                            specialization.target_name.clone(),
                                        );
                                    }
                                    let target = call_target_for_source(
                                        declaration.source,
                                        root_source,
                                        specialization.target_name.clone(),
                                    );
                                    definitions.insert(
                                        target,
                                        IndexedCallable::new_method(
                                            &method.callable,
                                            body,
                                            owner.target_ty(),
                                            method_specialization_context_substitutions(
                                                owner,
                                                specialization,
                                                &file.resolved,
                                                &resolved_sources,
                                            ),
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
                            let definition = analysis
                                .semantic_db
                                .definition_at(destruct.keyword_span)
                                .expect("indexed destructor must have a semantic definition");
                            names.insert(definition, name.clone());
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
                                if let Some(arguments) = specialization.ordered_type_arguments() {
                                    names.insert_instance(
                                        crate::mir::CallInstanceKey::from_types(
                                            def_id,
                                            Some(&specialization.self_ty),
                                            arguments,
                                        ),
                                        specialization.target_name.clone(),
                                    );
                                }
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
                            let (definition, declaration) =
                                canonical_callable_definition(analysis, function.member_name_span);
                            if function.generics.parameters.is_empty() {
                                let target = call_target_for_source(
                                    declaration.source,
                                    root_source,
                                    function.name.clone(),
                                );
                                names.insert(definition, function.name.clone());
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
                            for specialization in call_specializations
                                .functions
                                .get(&definition)
                                .into_iter()
                                .flatten()
                            {
                                if let Some(arguments) = specialization.ordered_type_arguments() {
                                    names.insert_instance(
                                        crate::mir::CallInstanceKey::from_types(
                                            definition, None, arguments,
                                        ),
                                        specialization.target_name.clone(),
                                    );
                                }
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
                        for (_, literal) in construct.literals() {
                            if literal.body.is_none() {
                                continue;
                            }
                            let (definition, declaration) =
                                canonical_callable_definition(analysis, literal.span);
                            for specialization in call_specializations
                                .literals
                                .get(&definition)
                                .into_iter()
                                .flatten()
                            {
                                names.insert_instance(
                                    literal_instance_key(specialization),
                                    specialization.target_name.clone(),
                                );
                                for method in specialization
                                    .pack_segments
                                    .iter()
                                    .filter_map(|segment| match segment {
                                        crate::analysis::literal_specializations::LiteralPackSegmentSpecialization::Spread {
                                            plan,
                                            ..
                                        } => Some(plan),
                                        crate::analysis::literal_specializations::LiteralPackSegmentSpecialization::Value {
                                            ..
                                        } => None,
                                    })
                                    .flat_map(|plan| {
                                        plan.conversion
                                            .iter()
                                            .chain([&plan.exact_size, &plan.step])
                                    })
                                {
                                    names.insert_instance(
                                        crate::mir::CallInstanceKey::from_types(
                                            file.resolved
                                                .callable_bodies
                                                .canonical_definition(method.def_id),
                                            Some(&method.self_ty),
                                            std::iter::empty(),
                                        ),
                                        method.target_name.clone(),
                                    );
                                }
                                let target = call_target_for_source(
                                    declaration.source,
                                    root_source,
                                    specialization.target_name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_literal(literal, specialization, file),
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
                let Some(plan) = file.typed_hir.closure_plan(closure_span).cloned() else {
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

        for file in &analysis.files {
            index_typed_hir_callable_names(&mut names, &file.typed_hir, analysis);
        }
        for callable in definitions.values() {
            if callable.substitutions.is_empty() {
                continue;
            }
            let specialized = callable.typed_hir.specialized(&callable.substitutions);
            index_typed_hir_callable_names(&mut names, &specialized, analysis);
        }

        for specializations in call_specializations.methods.values() {
            for specialization in specializations {
                let Some(arguments) = specialization.ordered_type_arguments() else {
                    continue;
                };
                names.insert_instance(
                    crate::mir::CallInstanceKey::from_types(
                        analysis
                            .callable_bodies
                            .canonical_definition(specialization.def_id),
                        Some(&specialization.self_ty),
                        arguments,
                    ),
                    specialization.target_name.clone(),
                );
            }
        }
        for specializations in call_specializations.functions.values() {
            for specialization in specializations {
                let Some(arguments) = specialization.ordered_type_arguments() else {
                    continue;
                };
                names.insert_instance(
                    crate::mir::CallInstanceKey::from_types(
                        analysis
                            .callable_bodies
                            .canonical_definition(specialization.def_id),
                        None,
                        arguments,
                    ),
                    specialization.target_name.clone(),
                );
            }
        }
        for plans in call_specializations.coercions.values() {
            for plan in plans {
                let Some(definition) = plan.def_id else {
                    continue;
                };
                names.insert_instance(
                    crate::mir::CallInstanceKey::from_types(
                        analysis.callable_bodies.canonical_definition(definition),
                        Some(&plan.self_ty),
                        std::iter::empty(),
                    ),
                    crate::ir::coercion_symbol_name(plan),
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
    mir_parameters: Option<Vec<Parameter>>,
    literal_pack: Option<crate::mir::LiteralPackInput>,
    literal_instance: Option<crate::mir::CallInstanceKey>,
    closure_mir: Option<ClosureMir<'a>>,
    return_type: Option<TypeExpr>,
    substitutions: HashMap<String, TypeExpr>,
    resolved: &'a ResolveOutput,
    typed_hir: &'a TypedHir,
    issues: Vec<BuildabilityIssue>,
}

#[derive(Clone)]
struct ClosureMir<'a> {
    expression: &'a crate::ast::ClosureExpr,
    plan: crate::typecheck::TypecheckClosurePlan,
    receiver_mode: crate::ast::MethodReceiverMode,
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
            mir_parameters: Some(Vec::new()),
            literal_pack: None,
            literal_instance: None,
            closure_mir: None,
            return_type: Some(test.return_type()),
            substitutions: HashMap::new(),
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
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
            mir_parameters: Some(function.parameters.parameters.clone()),
            literal_pack: None,
            literal_instance: None,
            closure_mir: None,
            return_type: Some(function.return_type.clone()),
            substitutions: HashMap::new(),
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
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
            mir_parameters: Some(
                function
                    .parameters
                    .parameters
                    .iter()
                    .cloned()
                    .map(|mut parameter| {
                        parameter.ty =
                            substitute_type_expr_parameters(&parameter.ty, &substitutions);
                        parameter
                    })
                    .collect(),
            ),
            literal_pack: None,
            literal_instance: None,
            closure_mir: None,
            return_type: Some(return_type),
            substitutions,
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
            issues,
        }
    }

    fn new_method(
        method: &'a CallableDecl,
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
        let concrete_self_ty = substitute_type_expr_parameters(self_ty, &substitutions);
        let mir_parameters = crate::callable_parameters::instance(
            method,
            &concrete_self_ty,
            &contextual_substitutions,
        );
        issues.extend(unsupported_outcome_return_type_issue(
            &return_type,
            method.return_type.span(),
            &file.resolved,
            resolved_sources,
        ));

        Self {
            span: method.span,
            body,
            mir_parameters: Some(mir_parameters),
            literal_pack: None,
            literal_instance: None,
            closure_mir: None,
            return_type: Some(return_type),
            substitutions: contextual_substitutions,
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
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
            mir_parameters: Some(vec![Parameter {
                span: drop_.binding.span,
                name: drop_.binding.name.clone(),
                name_span: drop_.binding.name_span,
                ty: substitute_type_expr_parameters(&drop_.binding.ty, &contextual_substitutions),
            }]),
            literal_pack: None,
            literal_instance: None,
            closure_mir: None,
            return_type: Some(TypeExpr::Reference(TypeReference {
                span: drop_.span,
                name: "void".to_string(),
            })),
            substitutions: contextual_substitutions,
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
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
            mir_parameters: None,
            literal_pack: None,
            literal_instance: None,
            closure_mir: Some(ClosureMir {
                expression,
                plan: plan.clone(),
                receiver_mode,
            }),
            return_type: Some((*plan.ty.return_type).clone()),
            substitutions: HashMap::new(),
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
            issues,
        }
    }

    fn new_literal(
        declaration: &'a crate::ast::LiteralDecl,
        specialization: &crate::analysis::literal_specializations::LiteralSpecialization,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            span: declaration.span,
            body: declaration
                .body
                .as_ref()
                .expect("buildability indexes only body-bearing literals"),
            mir_parameters: Some(literal_parameters(declaration, specialization)),
            literal_pack: literal_pack_input(declaration, specialization),
            literal_instance: Some(literal_instance_key(specialization)),
            closure_mir: None,
            return_type: Some(specialization.result_type.clone()),
            substitutions: specialization.substitutions.clone(),
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
            issues: Vec::new(),
        }
    }
}

fn literal_parameters(
    declaration: &crate::ast::LiteralDecl,
    specialization: &crate::analysis::literal_specializations::LiteralSpecialization,
) -> Vec<Parameter> {
    match specialization.shape {
        crate::ast::LiteralShape::Sequence => specialization
            .argument_types
            .iter()
            .enumerate()
            .map(|(index, ty)| Parameter {
                span: declaration
                    .capture
                    .as_ref()
                    .map_or(declaration.shape_span, |capture| capture.span),
                name: crate::analysis::literal_specializations::literal_element_parameter_name(
                    index,
                ),
                name_span: declaration
                    .capture
                    .as_ref()
                    .map_or(declaration.shape_span, |capture| capture.name_span),
                ty: ty.clone(),
            })
            .collect(),
        crate::ast::LiteralShape::String => declaration
            .parameters
            .parameters
            .iter()
            .zip(&specialization.argument_types)
            .map(|(parameter, ty)| Parameter {
                span: parameter.span,
                name: parameter.name.clone(),
                name_span: parameter.name_span,
                ty: ty.clone(),
            })
            .collect(),
    }
}

fn literal_pack_input(
    declaration: &crate::ast::LiteralDecl,
    specialization: &crate::analysis::literal_specializations::LiteralSpecialization,
) -> Option<crate::mir::LiteralPackInput> {
    let capture = declaration.capture.as_ref()?;
    let element_type = specialization.element_type.as_ref()?;
    (specialization.shape == crate::ast::LiteralShape::Sequence).then(|| {
        crate::mir::LiteralPackInput {
            capture_name: capture.name.clone(),
            capture_span: capture.span,
            element_type: element_type.clone(),
            segments: specialization
                .pack_segments
                .iter()
                .map(|segment| match segment {
                    crate::analysis::literal_specializations::LiteralPackSegmentSpecialization::Value {
                        parameter_index,
                    } => crate::mir::LiteralPackInputSegment::Value {
                        parameter_index: *parameter_index,
                    },
                    crate::analysis::literal_specializations::LiteralPackSegmentSpecialization::Spread {
                        iterator_parameter_index,
                        plan,
                    } => crate::mir::LiteralPackInputSegment::Spread {
                        parameter_index: *iterator_parameter_index,
                        plan: plan.clone(),
                    },
                })
                .collect(),
        }
    })
}

fn literal_instance_key(
    specialization: &crate::analysis::literal_specializations::LiteralSpecialization,
) -> crate::mir::CallInstanceKey {
    crate::mir::CallInstanceKey::from_literal_types(
        specialization.def_id,
        specialization.shape,
        &specialization.result_type,
        specialization.pack_segments.iter().map(|segment| {
            match segment {
            crate::analysis::literal_specializations::LiteralPackSegmentSpecialization::Value {
                ..
            } => (None, None),
            crate::analysis::literal_specializations::LiteralPackSegmentSpecialization::Spread {
                plan,
                ..
            } => (Some(plan.mode), Some(&plan.iterator_type)),
        }
        }),
    )
}

fn canonical_callable_definition(
    analysis: &CompileUnitAnalysis,
    location: ByteSpan,
) -> (DefId, ByteSpan) {
    let authored = analysis
        .semantic_db
        .definition_at(location)
        .expect("buildable callable must have a semantic definition");
    let definition = analysis.callable_bodies.canonical_definition(authored);
    let anchor = analysis
        .semantic_db
        .definition_anchor(definition)
        .expect("buildable callable definition must have a source anchor");
    (definition, anchor)
}

mod closures;
mod diagnostics;
mod signatures;
mod traversal;
mod types;

use diagnostics::*;
use signatures::*;
use traversal::*;
use types::*;

#[cfg(test)]
mod tests;
