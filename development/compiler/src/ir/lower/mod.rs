mod aggregates;
mod allocation_contexts;
mod bindings;
mod closures;
mod coercion_symbols;
mod collection_for;
mod context;
mod control_flow;
mod entry;
mod errors;
mod expressions;
mod functions;
mod imported_calls;
mod interpolation;
mod literal_pack_lengths;
mod literal_packs;
mod literals;
mod mir;
mod outcome_propagation;
mod outcome_values;
mod reachability;
mod regions;
mod typed_literals;
mod types;

#[cfg(test)]
mod tests;

use super::{CallTarget, Function, IrModule};
use crate::abi::{
    ReturnPassing, function_parameter_abi_word_count_from_signature_with_resolver,
    function_success_return_passing_from_signature_with_resolver,
};
use crate::analysis::{
    CompileUnitAnalysis, FileAnalysis,
    call_specializations::collect_call_specializations,
    literal_specializations::{LiteralSpecialization, literal_element_parameter_name},
};
use crate::ast::{
    CallableDecl, DestructDecl, FunctionDecl, GenericType, Item, LiteralDecl, LiteralShape,
    MethodDecl, Parameter, Stmt, TypeExpr, TypeReference, canonical_type_expr,
    substitute_type_expr_parameters,
};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::ir::Type;
use crate::resolve::{
    FunctionSignature as ResolvedFunctionSignature, ParameterSignature, ResolveOutput,
};
use crate::semantic::DefId;
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::test_entry::TestDeclarationId;
use crate::typecheck::TypedHir;
use context::{
    ErrorPayloads, FunctionNames, FunctionSignature, FunctionSignatures, ResolvedSources,
};
use imported_calls::{imported_call_diagnostics, imported_call_diagnostics_for_block};
use reachability::reachable_call_targets;
use std::collections::{HashMap, HashSet};
use types::{
    parameter_type_from_type_expr_with_resolver, return_type_from_type_expr_with_resolver,
    type_expr_with_self_type,
};

pub(crate) fn lower_executable(
    analysis: &CompileUnitAnalysis,
    sources: &SourceMap,
) -> Result<IrModule, Vec<Diagnostic>> {
    lower_process_entry(analysis, sources, None)
}

pub(crate) fn lower_test(
    analysis: &CompileUnitAnalysis,
    sources: &SourceMap,
    test: &TestDeclarationId,
) -> Result<IrModule, Vec<Diagnostic>> {
    lower_process_entry(analysis, sources, Some(test))
}

fn lower_process_entry(
    analysis: &CompileUnitAnalysis,
    sources: &SourceMap,
    test: Option<&TestDeclarationId>,
) -> Result<IrModule, Vec<Diagnostic>> {
    let Some(root) = analysis.root_file() else {
        return Err(vec![Diagnostic::error(
            "E8000",
            "IR lowering requires a root source file",
        )]);
    };

    let selected_test = if let Some(test_id) = test {
        let Some(test_decl) = test_id.resolve(&root.ast) else {
            return Err(vec![
                Diagnostic::error(
                    "E8000",
                    format!(
                        "IR lowering cannot resolve selected test `{}`",
                        test_id.name()
                    ),
                )
                .with_primary_span_if_absent(sources, root.ast.span),
            ]);
        };
        Some(test_decl)
    } else {
        None
    };
    let entry_contract = if selected_test.is_none() {
        root.ast.items.iter().find_map(|item| match item {
            Item::Function(function) if function.name == DEFAULT_ENTRY_NAME => Some(function),
            _ => None,
        })
    } else {
        None
    };
    if selected_test.is_none() && entry_contract.is_none() {
        return Err(vec![
            Diagnostic::error(
                "E8000",
                format!("IR lowering requires entry function `{DEFAULT_ENTRY_NAME}`"),
            )
            .with_primary_span_if_absent(sources, root.ast.span),
        ]);
    }

    let entry_definition = entry_contract.map(|contract| {
        if contract.body.is_some() {
            return (root, contract);
        }
        source_backed_entry_definition(analysis, contract).unwrap_or((root, contract))
    });

    let function_index = FunctionIndex::new(analysis, root.ast.span.source);
    let diagnostics = match selected_test {
        Some(test) => imported_call_diagnostics_for_block(
            sources,
            &test.body,
            root.ast.span.source,
            &root.resolved,
        ),
        None => imported_call_diagnostics(
            sources,
            entry_definition.expect("executable entry was validated").1,
            entry_definition
                .expect("executable entry was validated")
                .0
                .ast
                .span
                .source,
            &entry_definition
                .expect("executable entry was validated")
                .0
                .resolved,
        ),
    };
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let function_signatures = function_index.signatures();
    let function_names = function_index.names();
    let error_payloads = function_index.error_payloads(root.ast.span.source);
    let resolved_sources = function_index.resolved_sources();
    let selected_target = test.map_or_else(
        || CallTarget::same_file(DEFAULT_ENTRY_NAME),
        |test| CallTarget::same_file(format!("__nocter_test_entry_{}", test.item_index())),
    );
    let lowered_entry = match selected_test {
        Some(test) => entry::lower_test_entry_function(
            test,
            selected_target,
            sources,
            function_signatures.clone(),
            function_names.clone(),
            root.ast.span.source,
            &root.resolved,
            &root.typed_hir,
            resolved_sources.clone(),
            error_payloads.clone(),
        )
        .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, test.span))?,
        None => {
            let (entry_file, entry) = entry_definition.expect("executable entry was validated");
            entry::lower_entry_function_with_target(
                entry,
                selected_target,
                sources,
                function_signatures.clone(),
                function_names.clone(),
                entry_file.ast.span.source,
                &entry_file.resolved,
                &entry_file.typed_hir,
                resolved_sources.clone(),
                error_payloads.clone(),
            )
            .map_err(|diagnostics| {
                attach_primary_span_if_absent(diagnostics, sources, entry.span)
            })?
        }
    };
    let entry_target = lowered_entry.target.clone();
    let mut functions = vec![lowered_entry];
    lower_reachable_functions(
        &mut functions,
        &function_index,
        &function_signatures,
        &function_names,
        &error_payloads,
        &resolved_sources,
        root.ast.span.source,
        entry_target.clone(),
        sources,
    )?;

    Ok(IrModule::with_entry(entry_target, functions))
}

fn source_backed_entry_definition<'a>(
    analysis: &'a CompileUnitAnalysis,
    contract: &FunctionDecl,
) -> Option<(&'a FileAnalysis, &'a FunctionDecl)> {
    let authored = analysis.semantic_db.definition_at(contract.name_span)?;
    let declaration = analysis.callable_bodies.canonical_definition(authored);
    let implementation = analysis.callable_bodies.implementation_id(declaration)?;
    let implementation = analysis.semantic_db.definition_anchor(implementation)?;
    let file = analysis.file_by_source(implementation.source)?;
    let function = file.ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name_span == implementation => Some(function),
        _ => None,
    })?;
    Some((file, function))
}

fn lower_signature_return_type(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<Type> {
    return_type_from_type_expr_with_resolver(ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
}

fn lower_signature_parameter_type(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<Type> {
    parameter_type_from_type_expr_with_resolver(ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
}

fn lower_reachable_functions(
    lowered: &mut Vec<Function>,
    function_index: &FunctionIndex<'_>,
    function_signatures: &FunctionSignatures,
    function_names: &FunctionNames,
    error_payloads: &ErrorPayloads,
    resolved_sources: &ResolvedSources<'_>,
    root_source: SourceId,
    entry_target: CallTarget,
    sources: &SourceMap,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = HashSet::from([entry_target]);
    let mut queue = reachable_call_targets(&lowered[0]);

    while let Some(target) = queue.pop_front() {
        if !seen.insert(target.clone()) {
            continue;
        }

        let Some(function) = function_index.definition(&target) else {
            return Err(vec![Diagnostic::error(
                "E8006",
                format!(
                    "native lowering cannot find reachable function target `{}`",
                    describe_call_target(&target)
                ),
            )]);
        };
        let diagnostics = function.imported_call_diagnostics(sources, root_source);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let function = function.lower(
            target,
            sources,
            function_signatures.clone(),
            function_names.clone(),
            error_payloads.clone(),
            resolved_sources.clone(),
            root_source,
        )?;
        queue.extend(reachable_call_targets(&function));
        lowered.push(function);
    }

    Ok(())
}

struct FunctionIndex<'a> {
    definitions: HashMap<CallTarget, IndexedCallable<'a>>,
    resolved_sources: ResolvedSources<'a>,
    method_target_aliases: Vec<(String, CallTarget)>,
    semantic_db: &'a crate::semantic::SemanticDb,
    callable_bodies: &'a crate::callable_bodies::CallableBodyIndex,
    root_source: SourceId,
}

struct IndexedCallable<'a> {
    declaration: IndexedDeclaration<'a>,
    resolved: &'a ResolveOutput,
    typed_hir: &'a TypedHir,
}

enum IndexedDeclaration<'a> {
    Function {
        declaration: &'a FunctionDecl,
        substitutions: HashMap<String, TypeExpr>,
        name: String,
    },
    Drop {
        declaration: &'a DestructDecl,
        self_ty: TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        name: String,
    },
    Method {
        declaration: &'a CallableDecl,
        anchor: ByteSpan,
        self_ty: TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        name: String,
    },
    Literal {
        declaration: &'a LiteralDecl,
        specialization: LiteralSpecialization,
    },
    Closure {
        expression: &'a crate::ast::ClosureExpr,
        plan: crate::typecheck::TypecheckClosurePlan,
        receiver_mode: crate::ast::MethodReceiverMode,
        name: String,
    },
}

impl<'a> FunctionIndex<'a> {
    fn new(analysis: &'a CompileUnitAnalysis, root_source: SourceId) -> Self {
        let mut definitions = HashMap::new();
        let resolved_sources = analysis
            .files
            .iter()
            .map(|file| (file.ast.span.source, &file.resolved))
            .collect();
        let call_specializations = collect_call_specializations(analysis);
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
                        let (_, declaration_source) =
                            canonical_callable_definition(analysis, identity);
                        let target = call_target_for_source(
                            declaration_source,
                            root_source,
                            function.name.clone(),
                        );
                        definitions.insert(target, IndexedCallable::new_function(function, file));
                    }
                    Item::Function(function) if function.body.is_some() => {
                        let (definition, declaration_source) =
                            canonical_callable_definition(analysis, function.member_name_span);
                        for specialization in call_specializations
                            .functions
                            .get(&definition)
                            .into_iter()
                            .flatten()
                        {
                            let target = call_target_for_source(
                                declaration_source,
                                root_source,
                                specialization.target_name.clone(),
                            );
                            definitions.insert(
                                target,
                                IndexedCallable::new_function_specialization(
                                    function,
                                    specialization.substitutions.clone(),
                                    specialization.target_name.clone(),
                                    file,
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
                                let (_, declaration_source) =
                                    canonical_callable_definition(analysis, anchor);
                                let name = method_target_name(type_name, method_name);
                                let target = call_target_for_source(
                                    declaration_source,
                                    root_source,
                                    name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_instance_callable(
                                        method,
                                        anchor,
                                        owner.target_ty().clone(),
                                        HashMap::new(),
                                        name,
                                        file,
                                    ),
                                );
                            } else if method.body.is_some() {
                                let (def_id, declaration_source) =
                                    canonical_callable_definition(analysis, anchor);
                                for specialization in call_specializations
                                    .methods
                                    .get(&def_id)
                                    .into_iter()
                                    .flatten()
                                {
                                    let target = call_target_for_source(
                                        declaration_source,
                                        root_source,
                                        specialization.target_name.clone(),
                                    );
                                    definitions.insert(
                                        target,
                                        IndexedCallable::new_instance_callable(
                                            method,
                                            anchor,
                                            substitute_type_expr_parameters(
                                                owner.target_ty(),
                                                &specialization.substitutions,
                                            ),
                                            specialization.substitutions.clone(),
                                            specialization.target_name.clone(),
                                            file,
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
                            let (def_id, declaration_source) =
                                canonical_callable_definition(analysis, entry.as_span);
                            for plan in call_specializations
                                .coercions
                                .get(&def_id)
                                .into_iter()
                                .flatten()
                            {
                                let target_name = coercion_symbols::coercion_symbol_name(plan);
                                let target = call_target_for_source(
                                    declaration_source,
                                    root_source,
                                    target_name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_coercion(entry, plan, target_name, file),
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
                                let (_, declaration_source) =
                                    canonical_callable_definition(analysis, method.name_span);
                                let name = method_target_name(type_name, &method.name);
                                let target = call_target_for_source(
                                    declaration_source,
                                    root_source,
                                    name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_method(
                                        method,
                                        owner.target_ty().clone(),
                                        HashMap::new(),
                                        name,
                                        file,
                                    ),
                                );
                            } else if method.body.is_some() {
                                let (def_id, declaration_source) =
                                    canonical_callable_definition(analysis, method.name_span);
                                for specialization in call_specializations
                                    .methods
                                    .get(&def_id)
                                    .into_iter()
                                    .flatten()
                                {
                                    let target = call_target_for_source(
                                        declaration_source,
                                        root_source,
                                        specialization.target_name.clone(),
                                    );
                                    definitions.insert(
                                        target,
                                        IndexedCallable::new_method(
                                            method,
                                            substitute_type_expr_parameters(
                                                owner.target_ty(),
                                                &specialization.substitutions,
                                            ),
                                            specialization.substitutions.clone(),
                                            specialization.target_name.clone(),
                                            file,
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
                            definitions.insert(
                                target,
                                IndexedCallable::new_drop(
                                    destruct,
                                    destruct.target_ty.clone(),
                                    HashMap::new(),
                                    name,
                                    file,
                                ),
                            );
                        } else {
                            let definition = analysis
                                .semantic_db
                                .definition_at(destruct.keyword_span)
                                .expect("lowered destructor must have a semantic definition");
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
                                        specialization.self_ty.clone(),
                                        specialization.substitutions.clone(),
                                        specialization.target_name.clone(),
                                        file,
                                    ),
                                );
                            }
                        }
                    }
                    Item::Interface(interface) => {
                        for method in &interface.methods {
                            if method.body.is_none() {
                                continue;
                            }
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
                                let mut substitutions = specialization.substitutions.clone();
                                substitutions
                                    .insert("Self".to_string(), specialization.self_ty.clone());
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_method(
                                        method,
                                        specialization.self_ty.clone(),
                                        substitutions,
                                        specialization.target_name.clone(),
                                        file,
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
                            let (definition, declaration_source) =
                                canonical_callable_definition(analysis, function.member_name_span);
                            if function.generics.parameters.is_empty() {
                                let target = call_target_for_source(
                                    declaration_source,
                                    root_source,
                                    function.name.clone(),
                                );
                                definitions
                                    .insert(target, IndexedCallable::new_function(function, file));
                                continue;
                            }
                            for specialization in call_specializations
                                .functions
                                .get(&definition)
                                .into_iter()
                                .flatten()
                            {
                                let target = call_target_for_source(
                                    declaration_source,
                                    root_source,
                                    specialization.target_name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_function_specialization(
                                        function,
                                        specialization.substitutions.clone(),
                                        specialization.target_name.clone(),
                                        file,
                                    ),
                                );
                            }
                        }
                        for (_, literal) in construct.literals() {
                            if literal.body.is_none() {
                                continue;
                            }
                            let (definition, declaration_source) =
                                canonical_callable_definition(analysis, literal.span);
                            for specialization in call_specializations
                                .literals
                                .get(&definition)
                                .into_iter()
                                .flatten()
                            {
                                let target = call_target_for_source(
                                    declaration_source,
                                    root_source,
                                    specialization.target_name.clone(),
                                );
                                definitions.insert(
                                    target,
                                    IndexedCallable::new_literal(
                                        literal,
                                        specialization.clone(),
                                        file,
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
                        specialization.target_name.clone(),
                        file,
                    ),
                );
            }
        }
        let method_target_aliases = call_specializations
            .method_target_aliases
            .iter()
            .map(|alias| {
                (
                    alias.requested_name.clone(),
                    call_target_for_source(
                        alias.declaration_span.source,
                        root_source,
                        alias.target_name.clone(),
                    ),
                )
            })
            .collect();
        Self {
            definitions,
            resolved_sources,
            method_target_aliases,
            semantic_db: &analysis.semantic_db,
            callable_bodies: &analysis.callable_bodies,
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

    fn signatures(&self) -> FunctionSignatures {
        FunctionSignatures::from_call_targets(
            self.definitions
                .iter()
                .flat_map(|(target, function)| {
                    let Some(signature) = function.signature(&self.resolved_sources) else {
                        return Vec::new();
                    };
                    let mut entries = vec![(target.clone(), signature.clone())];
                    if let CallTarget::Imported { source, name } = target
                        && let Some(resolved) = self.resolved_sources.get(source)
                    {
                        let module = resolved.module_source(*source);
                        if module != *source {
                            entries.push((
                                call_target_for_source(module, self.root_source, name.clone()),
                                signature,
                            ));
                        }
                    }
                    entries
                })
                .collect(),
        )
    }

    fn resolved_sources(&self) -> ResolvedSources<'a> {
        self.resolved_sources.clone()
    }

    fn names(&self) -> FunctionNames {
        FunctionNames::from_index(
            self.definitions
                .values()
                .filter_map(|function| function.name_declaration())
                .filter_map(|(span, name)| {
                    let authored = self.semantic_db.definition_at(span)?;
                    let definition = self.callable_bodies.canonical_definition(authored);
                    Some((definition, name))
                })
                .collect(),
            self.definitions
                .keys()
                .map(|target| (call_target_name(target).to_string(), target.clone()))
                .chain(self.method_target_aliases.iter().cloned())
                .collect(),
        )
    }

    fn error_payloads(&self, root_source: SourceId) -> ErrorPayloads {
        self.definitions
            .iter()
            .filter_map(|(target, function)| {
                function
                    .static_error_payload(root_source)
                    .map(|payload| (target.clone(), payload))
            })
            .collect()
    }
}

impl<'a> IndexedCallable<'a> {
    fn new_function(declaration: &'a FunctionDecl, file: &'a FileAnalysis) -> Self {
        let mut substitutions = HashMap::new();
        insert_function_self_substitution(declaration, &file.resolved, &mut substitutions);
        Self {
            declaration: IndexedDeclaration::Function {
                declaration,
                substitutions,
                name: declaration.name.clone(),
            },
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
        }
    }

    fn new_function_specialization(
        declaration: &'a FunctionDecl,
        mut substitutions: HashMap<String, TypeExpr>,
        name: String,
        file: &'a FileAnalysis,
    ) -> Self {
        insert_function_self_substitution(declaration, &file.resolved, &mut substitutions);
        Self {
            declaration: IndexedDeclaration::Function {
                declaration,
                substitutions,
                name,
            },
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
        }
    }

    fn new_drop(
        declaration: &'a DestructDecl,
        self_ty: TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        name: String,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            declaration: IndexedDeclaration::Drop {
                declaration,
                self_ty,
                substitutions,
                name,
            },
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
        }
    }

    fn new_method(
        declaration: &'a MethodDecl,
        self_ty: TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        name: String,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            declaration: IndexedDeclaration::Method {
                declaration: &declaration.callable,
                anchor: declaration.name_span,
                self_ty,
                substitutions,
                name,
            },
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
        }
    }

    fn new_instance_callable(
        declaration: &'a CallableDecl,
        anchor: ByteSpan,
        self_ty: TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        name: String,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            declaration: IndexedDeclaration::Method {
                declaration,
                anchor,
                self_ty,
                substitutions,
                name,
            },
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
        }
    }

    fn new_coercion(
        declaration: &'a crate::ast::CoercionEntry,
        plan: &crate::typecheck::TypecheckCoercionPlan,
        name: String,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            declaration: IndexedDeclaration::Method {
                declaration: declaration.callable(),
                anchor: declaration.as_span,
                self_ty: plan.self_ty.clone(),
                substitutions: plan.substitutions.clone(),
                name,
            },
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
        }
    }

    fn new_literal(
        declaration: &'a LiteralDecl,
        specialization: LiteralSpecialization,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            declaration: IndexedDeclaration::Literal {
                declaration,
                specialization,
            },
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
        }
    }

    fn new_closure(
        expression: &'a crate::ast::ClosureExpr,
        plan: crate::typecheck::TypecheckClosurePlan,
        receiver_mode: crate::ast::MethodReceiverMode,
        name: String,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            declaration: IndexedDeclaration::Closure {
                expression,
                plan,
                receiver_mode,
                name,
            },
            resolved: &file.resolved,
            typed_hir: &file.typed_hir,
        }
    }

    fn imported_call_diagnostics(
        &self,
        sources: &SourceMap,
        root_source: SourceId,
    ) -> Vec<Diagnostic> {
        match &self.declaration {
            IndexedDeclaration::Function { declaration, .. } => {
                imported_call_diagnostics(sources, declaration, root_source, self.resolved)
            }
            IndexedDeclaration::Drop { declaration, .. } => {
                imported_calls::imported_call_diagnostics_for_block(
                    sources,
                    &declaration.body,
                    root_source,
                    self.resolved,
                )
            }
            IndexedDeclaration::Method { declaration, .. } => declaration
                .body
                .as_ref()
                .map(|body| {
                    imported_calls::imported_call_diagnostics_for_block(
                        sources,
                        body,
                        root_source,
                        self.resolved,
                    )
                })
                .unwrap_or_default(),
            IndexedDeclaration::Literal { declaration, .. } => {
                declaration.body.as_ref().map_or_else(Vec::new, |body| {
                    imported_calls::imported_call_diagnostics_for_block(
                        sources,
                        body,
                        root_source,
                        self.resolved,
                    )
                })
            }
            IndexedDeclaration::Closure { expression, .. } => {
                imported_calls::imported_call_diagnostics_for_block(
                    sources,
                    &expression.body,
                    root_source,
                    self.resolved,
                )
            }
        }
    }

    fn static_error_payload(&self, root_source: SourceId) -> Option<errors::ErrorPayload> {
        let IndexedDeclaration::Function {
            declaration: function,
            ..
        } = &self.declaration
        else {
            return None;
        };
        if !function.parameters.parameters.is_empty() {
            return None;
        }
        let mut runtime_statements = function
            .body
            .as_ref()?
            .statements
            .iter()
            .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)));
        let Some(Stmt::Return(statement)) = runtime_statements.next() else {
            return None;
        };
        if runtime_statements.next().is_some() {
            return None;
        };
        let expression = statement.expression.as_ref()?;
        errors::lower_error_payload(expression, self.resolved, root_source, None)
            .ok()
            .flatten()
    }

    fn lower(
        &self,
        target: CallTarget,
        sources: &SourceMap,
        function_signatures: FunctionSignatures,
        function_names: FunctionNames,
        error_payloads: ErrorPayloads,
        resolved_sources: ResolvedSources<'a>,
        root_source: SourceId,
    ) -> Result<Function, Vec<Diagnostic>> {
        let span = self.declaration.span();
        match &self.declaration {
            IndexedDeclaration::Function {
                declaration,
                substitutions,
                name,
            } => functions::lower_function(
                declaration,
                substitutions,
                name.clone(),
                sources,
                target,
                function_signatures,
                function_names,
                root_source,
                self.resolved,
                self.typed_hir,
                resolved_sources,
                error_payloads,
            ),
            IndexedDeclaration::Drop {
                declaration,
                self_ty,
                substitutions,
                name,
            } => functions::lower_drop_function(
                declaration,
                self_ty,
                substitutions,
                name.clone(),
                sources,
                target,
                function_signatures,
                function_names,
                root_source,
                self.resolved,
                self.typed_hir,
                resolved_sources,
                error_payloads,
            ),
            IndexedDeclaration::Method {
                declaration,
                self_ty,
                substitutions,
                name,
                ..
            } => functions::lower_method_function(
                declaration,
                self_ty,
                substitutions,
                name.clone(),
                sources,
                target,
                function_signatures,
                function_names,
                root_source,
                self.resolved,
                self.typed_hir,
                resolved_sources,
                error_payloads,
            ),
            IndexedDeclaration::Literal {
                declaration,
                specialization,
            } => functions::lower_literal_function(
                declaration,
                specialization,
                sources,
                target,
                function_signatures,
                function_names,
                root_source,
                self.resolved,
                self.typed_hir,
                resolved_sources,
                error_payloads,
            ),
            IndexedDeclaration::Closure {
                expression,
                plan,
                receiver_mode,
                name,
            } => closures::lower_closure_function(
                expression,
                plan,
                *receiver_mode,
                name.clone(),
                sources,
                target,
                function_signatures,
                function_names,
                root_source,
                self.resolved,
                self.typed_hir,
                resolved_sources,
                error_payloads,
            ),
        }
        .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, span))
    }

    fn signature(&self, resolved_sources: &ResolvedSources<'a>) -> Option<FunctionSignature> {
        match &self.declaration {
            IndexedDeclaration::Function {
                declaration: function,
                substitutions,
                ..
            } => {
                let mut contextual_substitutions = substitutions.clone();
                crate::typecheck::extend_associated_type_substitutions_with_resolver(
                    &mut contextual_substitutions,
                    self.resolved,
                    |source| resolved_sources.get(&source).copied(),
                );
                let parameters = function_parameters(function, &contextual_substitutions);
                let return_type = substitute_type_expr_parameters(
                    &function.return_type,
                    &contextual_substitutions,
                );
                let resolved_signature =
                    resolved_function_signature(&parameters, return_type.clone());
                lower_signature_return_type(&return_type, self.resolved, resolved_sources).map(
                    |return_type| {
                        let parameter_types = parameters
                            .iter()
                            .map(|parameter| {
                                lower_signature_parameter_type(
                                    &parameter.ty,
                                    self.resolved,
                                    resolved_sources,
                                )
                            })
                            .collect::<Option<Vec<_>>>();

                        FunctionSignature {
                            return_type,
                            parameter_types,
                            parameter_abi_word_count: parameter_abi_word_count(
                                &resolved_signature,
                                self.resolved,
                                resolved_sources,
                            ),
                            success_return_passing: success_return_passing(
                                &resolved_signature,
                                self.resolved,
                                resolved_sources,
                            ),
                        }
                    },
                )
            }
            IndexedDeclaration::Drop {
                declaration,
                self_ty,
                substitutions,
                ..
            } => {
                let parameter_ty = substitute_type_expr_parameters(
                    &type_expr_with_self_type(&declaration.binding.ty, self_ty),
                    substitutions,
                );
                let parameter_type =
                    lower_signature_parameter_type(&parameter_ty, self.resolved, resolved_sources)?;
                let resolved_signature = resolved_function_signature(
                    &[Parameter {
                        span: declaration.binding.span,
                        name: declaration.binding.name.clone(),
                        name_span: declaration.binding.name_span,
                        ty: parameter_ty,
                    }],
                    void_type_expr(declaration.span),
                );
                Some(FunctionSignature {
                    return_type: Type::Void,
                    parameter_types: Some(vec![parameter_type]),
                    parameter_abi_word_count: parameter_abi_word_count(
                        &resolved_signature,
                        self.resolved,
                        resolved_sources,
                    ),
                    success_return_passing: success_return_passing(
                        &resolved_signature,
                        self.resolved,
                        resolved_sources,
                    ),
                })
            }
            IndexedDeclaration::Method {
                declaration,
                self_ty,
                substitutions,
                ..
            } => {
                let concrete_self_ty = substitute_type_expr_parameters(self_ty, substitutions);
                let mut contextual_substitutions = substitutions.clone();
                contextual_substitutions.insert("Self".to_string(), concrete_self_ty.clone());
                crate::typecheck::extend_associated_type_substitutions_with_resolver(
                    &mut contextual_substitutions,
                    self.resolved,
                    |source| resolved_sources.get(&source).copied(),
                );
                let parameters =
                    method_parameters(declaration, &concrete_self_ty, &contextual_substitutions);
                let return_type = substitute_type_expr_parameters(
                    &declaration.return_type,
                    &contextual_substitutions,
                );
                let resolved_signature =
                    resolved_function_signature(&parameters, return_type.clone());
                lower_signature_return_type(&return_type, self.resolved, resolved_sources).map(
                    |return_type| {
                        let parameter_types = parameters
                            .iter()
                            .map(|parameter| {
                                lower_signature_parameter_type(
                                    &parameter.ty,
                                    self.resolved,
                                    resolved_sources,
                                )
                            })
                            .collect::<Option<Vec<_>>>();

                        FunctionSignature {
                            return_type,
                            parameter_types,
                            parameter_abi_word_count: parameter_abi_word_count(
                                &resolved_signature,
                                self.resolved,
                                resolved_sources,
                            ),
                            success_return_passing: success_return_passing(
                                &resolved_signature,
                                self.resolved,
                                resolved_sources,
                            ),
                        }
                    },
                )
            }
            IndexedDeclaration::Literal {
                declaration,
                specialization,
            } => {
                let parameters = literal_parameters(declaration, specialization);
                let resolved_signature =
                    resolved_function_signature(&parameters, specialization.result_type.clone());
                lower_signature_return_type(
                    &specialization.result_type,
                    self.resolved,
                    resolved_sources,
                )
                .map(|return_type| {
                    let parameter_types = parameters
                        .iter()
                        .map(|parameter| {
                            lower_signature_parameter_type(
                                &parameter.ty,
                                self.resolved,
                                resolved_sources,
                            )
                        })
                        .collect::<Option<Vec<_>>>();
                    FunctionSignature {
                        return_type,
                        parameter_types,
                        parameter_abi_word_count: parameter_abi_word_count(
                            &resolved_signature,
                            self.resolved,
                            resolved_sources,
                        ),
                        success_return_passing: success_return_passing(
                            &resolved_signature,
                            self.resolved,
                            resolved_sources,
                        ),
                    }
                })
            }
            IndexedDeclaration::Closure {
                expression,
                plan,
                receiver_mode,
                ..
            } => closures::closure_function_signature(
                expression,
                plan,
                *receiver_mode,
                self.resolved,
                resolved_sources,
            ),
        }
    }

    fn name_declaration(&self) -> Option<(crate::source::ByteSpan, String)> {
        match &self.declaration {
            IndexedDeclaration::Function {
                declaration,
                substitutions,
                name,
            } if substitutions.is_empty() => Some((declaration.name_span, name.clone())),
            IndexedDeclaration::Function { .. } => None,
            IndexedDeclaration::Drop {
                declaration,
                substitutions,
                name,
                ..
            } if substitutions.is_empty() => Some((declaration.keyword_span, name.clone())),
            IndexedDeclaration::Drop { .. } => None,
            IndexedDeclaration::Method {
                declaration,
                anchor,
                substitutions,
                name,
                ..
            } if substitutions.is_empty() => Some((*anchor, name.clone())),
            IndexedDeclaration::Method { .. } => None,
            IndexedDeclaration::Literal { .. } => None,
            IndexedDeclaration::Closure { .. } => None,
        }
    }
}

fn insert_function_self_substitution(
    function: &FunctionDecl,
    resolved: &ResolveOutput,
    substitutions: &mut HashMap<String, TypeExpr>,
) {
    let Some(owner) = &function.owner else {
        return;
    };
    if crate::builtin_types::BuiltinTypeOwner::from_reference_name(&owner.name).is_some() {
        substitutions.insert(
            "Self".to_string(),
            TypeExpr::Reference(TypeReference {
                span: owner.name_span,
                name: owner.name.clone(),
            }),
        );
        return;
    }
    let Some(symbol) = resolved.type_symbol_by_name(&owner.name).or_else(|| {
        resolved
            .builtin_type_surface_for_name(&owner.name)
            .map(|surface| &surface.symbol)
    }) else {
        return;
    };
    let self_ty = if symbol.generic_parameters.is_empty() {
        TypeExpr::Reference(TypeReference {
            span: owner.name_span,
            name: owner.name.clone(),
        })
    } else {
        TypeExpr::Generic(GenericType {
            span: owner.name_span,
            name: owner.name.clone(),
            name_span: owner.name_span,
            arguments: symbol
                .generic_parameters
                .iter()
                .map(|parameter| {
                    substitutions.get(parameter).cloned().unwrap_or_else(|| {
                        TypeExpr::Reference(TypeReference {
                            span: owner.name_span,
                            name: parameter.clone(),
                        })
                    })
                })
                .collect(),
        })
    };
    substitutions.insert("Self".to_string(), self_ty);
}

fn literal_parameters(
    declaration: &LiteralDecl,
    specialization: &LiteralSpecialization,
) -> Vec<Parameter> {
    match specialization.shape {
        LiteralShape::Sequence => specialization
            .argument_types
            .iter()
            .enumerate()
            .map(|(index, ty)| Parameter {
                span: declaration
                    .capture
                    .as_ref()
                    .map_or(declaration.shape_span, |capture| capture.span),
                name: literal_element_parameter_name(index),
                name_span: declaration
                    .capture
                    .as_ref()
                    .map_or(declaration.shape_span, |capture| capture.name_span),
                ty: ty.clone(),
            })
            .collect(),
        LiteralShape::String => declaration
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

fn method_parameters(
    method: &CallableDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
) -> Vec<Parameter> {
    let mut parameters = Vec::with_capacity(method.parameters.parameters.len() + 1);
    parameters.push(Parameter {
        span: method.receiver.span,
        name: method.receiver.name.clone(),
        name_span: method.receiver.name_span,
        ty: substitute_type_expr_parameters(
            &type_expr_with_self_type(&method.receiver.implicit_parameter().ty, self_ty),
            substitutions,
        ),
    });
    parameters.extend(
        method
            .parameters
            .parameters
            .iter()
            .map(|parameter| Parameter {
                span: parameter.span,
                name: parameter.name.clone(),
                name_span: parameter.name_span,
                ty: substitute_type_expr_parameters(&parameter.ty, substitutions),
            }),
    );
    parameters
}

fn function_parameters(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
) -> Vec<Parameter> {
    function
        .parameters
        .parameters
        .iter()
        .map(|parameter| Parameter {
            span: parameter.span,
            name: parameter.name.clone(),
            name_span: parameter.name_span,
            ty: substitute_type_expr_parameters(&parameter.ty, substitutions),
        })
        .collect()
}

fn resolved_function_signature(
    parameters: &[Parameter],
    return_type: TypeExpr,
) -> ResolvedFunctionSignature {
    ResolvedFunctionSignature {
        generic_parameters: Vec::new(),
        generic_parameter_requirements: Vec::new(),
        where_clause: None,
        parameters: parameters
            .iter()
            .map(|parameter| ParameterSignature {
                name: parameter.name.clone(),
                name_span: parameter.name_span,
                ty: parameter.ty.clone(),
            })
            .collect(),
        return_type,
        result_provenance: None,
    }
}

fn parameter_abi_word_count(
    signature: &ResolvedFunctionSignature,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<usize> {
    function_parameter_abi_word_count_from_signature_with_resolver(signature, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
    .ok()
}

fn success_return_passing(
    signature: &ResolvedFunctionSignature,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<ReturnPassing> {
    function_success_return_passing_from_signature_with_resolver(signature, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
    .ok()
}

fn void_type_expr(span: ByteSpan) -> TypeExpr {
    TypeExpr::Reference(TypeReference {
        span,
        name: "void".to_string(),
    })
}

impl IndexedDeclaration<'_> {
    fn span(&self) -> ByteSpan {
        match self {
            IndexedDeclaration::Function { declaration, .. } => declaration.span,
            IndexedDeclaration::Drop { declaration, .. } => declaration.span,
            IndexedDeclaration::Method { declaration, .. } => declaration.span,
            IndexedDeclaration::Literal { declaration, .. } => declaration.span,
            IndexedDeclaration::Closure { expression, .. } => expression.span,
        }
    }
}

fn call_target_for_source(source: SourceId, root_source: SourceId, name: String) -> CallTarget {
    if source == root_source {
        CallTarget::same_file(name)
    } else {
        CallTarget::imported(source, name)
    }
}

fn canonical_callable_definition(
    analysis: &CompileUnitAnalysis,
    location: ByteSpan,
) -> (DefId, SourceId) {
    let authored = analysis
        .semantic_db
        .definition_at(location)
        .expect("lowered callable must have a semantic definition");
    let definition = analysis.callable_bodies.canonical_definition(authored);
    let source = analysis
        .semantic_db
        .definition_anchor(definition)
        .expect("lowered callable definition must have a source anchor")
        .source;
    (definition, source)
}

fn call_target_name(target: &CallTarget) -> &str {
    match target {
        CallTarget::SameFile(name) | CallTarget::Imported { name, .. } => name,
    }
}

fn method_target_name(type_name: &str, method_name: &str) -> String {
    format!("{type_name}.{method_name}")
}

fn drop_target_name(self_ty: &TypeExpr) -> String {
    format!("{}.drop", canonical_type_expr(self_ty))
}

fn declaration_target_type_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        TypeExpr::View(_) => Some("[]"),
        _ => None,
    }
}

fn attach_primary_span_if_absent(
    diagnostics: Vec<Diagnostic>,
    sources: &SourceMap,
    span: ByteSpan,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.with_primary_span_if_absent(sources, span))
        .collect()
}

fn describe_call_target(target: &CallTarget) -> String {
    match target {
        CallTarget::SameFile(name) => name.clone(),
        CallTarget::Imported { source, name } => {
            format!("{} from source {}", name, source.raw())
        }
    }
}
