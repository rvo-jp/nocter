mod aggregates;
mod allocation_contexts;
mod bindings;
mod context;
mod control_flow;
mod entry;
mod errors;
mod expressions;
mod functions;
mod imported_calls;
mod interpolation;
mod literal_packs;
mod literals;
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
    DropDecl, FunctionDecl, ImplMember, Item, LiteralDecl, LiteralShape, MethodDecl, Parameter,
    Stmt, TypeExpr, TypeReference, substitute_type_expr_parameters, type_expr_display_lossy,
};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::ir::Type;
use crate::resolve::{
    FunctionSignature as ResolvedFunctionSignature, ParameterSignature, ResolveOutput,
};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::typecheck::TypecheckFacts;
use context::{
    ErrorPayloads, FunctionNames, FunctionSignature, FunctionSignatures, ResolvedSources,
};
use imported_calls::imported_call_diagnostics;
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
    let Some(root) = analysis.root_file() else {
        return Err(vec![Diagnostic::error(
            "E8000",
            "IR lowering requires a root source file",
        )]);
    };

    let Some(entry) = root.ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == DEFAULT_ENTRY_NAME => Some(function),
        _ => None,
    }) else {
        return Err(vec![
            Diagnostic::error(
                "E8000",
                format!("IR lowering requires entry function `{DEFAULT_ENTRY_NAME}`"),
            )
            .with_primary_span_if_absent(sources, root.ast.span),
        ]);
    };

    let function_index = FunctionIndex::new(analysis, root.ast.span.source);
    let diagnostics =
        imported_call_diagnostics(sources, entry, root.ast.span.source, &root.resolved);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let function_signatures = function_index.signatures();
    let function_names = function_index.names();
    let error_payloads = function_index.error_payloads(root.ast.span.source);
    let resolved_sources = function_index.resolved_sources();
    let mut functions = vec![
        entry::lower_entry_function(
            entry,
            sources,
            function_signatures.clone(),
            function_names.clone(),
            root.ast.span.source,
            &root.resolved,
            &root.typecheck_facts,
            resolved_sources.clone(),
            error_payloads.clone(),
        )
        .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, entry.span))?,
    ];
    lower_reachable_functions(
        &mut functions,
        &function_index,
        &function_signatures,
        &function_names,
        &error_payloads,
        &resolved_sources,
        root.ast.span.source,
        sources,
    )?;

    Ok(IrModule::new(functions))
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
    sources: &SourceMap,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = HashSet::from([CallTarget::same_file(DEFAULT_ENTRY_NAME)]);
    let mut queue = reachable_call_targets(&lowered[0]);

    while let Some(target) = queue.pop_front() {
        if !seen.insert(target.clone()) {
            continue;
        }

        let Some(function) = function_index.definition(&target) else {
            return Err(vec![Diagnostic::error(
                "E8006",
                format!(
                    "IR v0 cannot find reachable function target `{}`",
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
}

struct IndexedCallable<'a> {
    declaration: IndexedDeclaration<'a>,
    resolved: &'a ResolveOutput,
    typecheck_facts: &'a TypecheckFacts,
}

enum IndexedDeclaration<'a> {
    Function {
        declaration: &'a FunctionDecl,
        substitutions: HashMap<String, TypeExpr>,
        name: String,
    },
    Drop {
        declaration: &'a DropDecl,
        self_ty: TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        name: String,
    },
    Method {
        declaration: &'a MethodDecl,
        self_ty: TypeExpr,
        substitutions: HashMap<String, TypeExpr>,
        name: String,
    },
    Literal {
        declaration: &'a LiteralDecl,
        specialization: LiteralSpecialization,
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
                    Item::Function(function) if function.generics.parameters.is_empty() => {
                        let target = call_target_for_source(
                            file.ast.span.source,
                            root_source,
                            function.name.clone(),
                        );
                        definitions.insert(target, IndexedCallable::new_function(function, file));
                    }
                    Item::Function(function) => {
                        for specialization in call_specializations
                            .functions
                            .get(&function.name_span)
                            .or_else(|| {
                                call_specializations
                                    .functions
                                    .get(&function.member_name_span)
                            })
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
                                IndexedCallable::new_function_specialization(
                                    function,
                                    specialization.substitutions.clone(),
                                    specialization.target_name.clone(),
                                    file,
                                ),
                            );
                        }
                    }
                    Item::Impl(impl_) if impl_.interface_ty.is_none() => {
                        let Some(type_name) = impl_target_type_name(&impl_.target_ty) else {
                            continue;
                        };
                        for member in &impl_.members {
                            match member {
                                ImplMember::Drop(drop_) if impl_.generics.parameters.is_empty() => {
                                    let name = drop_target_name(&impl_.target_ty);
                                    let target = call_target_for_source(
                                        file.ast.span.source,
                                        root_source,
                                        name.clone(),
                                    );
                                    definitions.insert(
                                        target,
                                        IndexedCallable::new_drop(
                                            drop_,
                                            impl_.target_ty.clone(),
                                            HashMap::new(),
                                            name,
                                            file,
                                        ),
                                    );
                                }
                                ImplMember::Drop(drop_) => {
                                    for specialization in call_specializations
                                        .drops
                                        .get(&drop_name_span(drop_.span))
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
                                                drop_,
                                                specialization.self_ty.clone(),
                                                specialization.substitutions.clone(),
                                                specialization.target_name.clone(),
                                                file,
                                            ),
                                        );
                                    }
                                }
                                ImplMember::Method(method)
                                    if method.body.is_some()
                                        && impl_.generics.parameters.is_empty() =>
                                {
                                    let name = method_target_name(type_name, &method.name);
                                    let target = call_target_for_source(
                                        file.ast.span.source,
                                        root_source,
                                        name.clone(),
                                    );
                                    definitions.insert(
                                        target,
                                        IndexedCallable::new_method(
                                            method,
                                            impl_.target_ty.clone(),
                                            HashMap::new(),
                                            name,
                                            file,
                                        ),
                                    );
                                }
                                ImplMember::Method(method) if method.body.is_some() => {
                                    for specialization in call_specializations
                                        .methods
                                        .get(&method.name_span)
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
                                                substitute_type_expr_parameters(
                                                    &impl_.target_ty,
                                                    &specialization.substitutions,
                                                ),
                                                specialization.substitutions.clone(),
                                                specialization.target_name.clone(),
                                                file,
                                            ),
                                        );
                                    }
                                }
                                ImplMember::Method(_) => {}
                            }
                        }
                    }
                    Item::Literal(literal) => {
                        for specialization in call_specializations
                            .literals
                            .get(&literal.span)
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
                                IndexedCallable::new_literal(literal, specialization.clone(), file),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        Self {
            definitions,
            resolved_sources,
        }
    }

    fn definition(&self, target: &CallTarget) -> Option<&IndexedCallable<'a>> {
        self.definitions.get(target)
    }

    fn signatures(&self) -> FunctionSignatures {
        FunctionSignatures::from_call_targets(
            self.definitions
                .iter()
                .filter_map(|(target, function)| {
                    function
                        .signature(&self.resolved_sources)
                        .map(|signature| (target.clone(), signature))
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
                .collect(),
            self.definitions
                .keys()
                .map(|target| (call_target_name(target).to_string(), target.clone()))
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
        Self {
            declaration: IndexedDeclaration::Function {
                declaration,
                substitutions: HashMap::new(),
                name: declaration.name.clone(),
            },
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
        }
    }

    fn new_function_specialization(
        declaration: &'a FunctionDecl,
        substitutions: HashMap<String, TypeExpr>,
        name: String,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            declaration: IndexedDeclaration::Function {
                declaration,
                substitutions,
                name,
            },
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
        }
    }

    fn new_drop(
        declaration: &'a DropDecl,
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
            typecheck_facts: &file.typecheck_facts,
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
                declaration,
                self_ty,
                substitutions,
                name,
            },
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
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
            typecheck_facts: &file.typecheck_facts,
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
                imported_calls::imported_call_diagnostics_for_block(
                    sources,
                    &declaration.body,
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
                self.typecheck_facts,
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
                self.typecheck_facts,
                resolved_sources,
                error_payloads,
            ),
            IndexedDeclaration::Method {
                declaration,
                self_ty,
                substitutions,
                name,
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
                self.typecheck_facts,
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
                self.typecheck_facts,
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
                let parameters = function_parameters(function, substitutions);
                let return_type =
                    substitute_type_expr_parameters(&function.return_type, substitutions);
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
                let parameters = method_parameters(declaration, self_ty, substitutions);
                let return_type = substitute_type_expr_parameters(
                    &type_expr_with_self_type(&declaration.return_type, self_ty),
                    substitutions,
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
            } if substitutions.is_empty() => Some((drop_name_span(declaration.span), name.clone())),
            IndexedDeclaration::Drop { .. } => None,
            IndexedDeclaration::Method {
                declaration,
                substitutions,
                name,
                ..
            } if substitutions.is_empty() => Some((declaration.name_span, name.clone())),
            IndexedDeclaration::Method { .. } => None,
            IndexedDeclaration::Literal { .. } => None,
        }
    }
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
    method: &MethodDecl,
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
        generic_parameter_bounds: Vec::new(),
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

fn call_target_name(target: &CallTarget) -> &str {
    match target {
        CallTarget::SameFile(name) | CallTarget::Imported { name, .. } => name,
    }
}

fn method_target_name(type_name: &str, method_name: &str) -> String {
    format!("{type_name}.{method_name}")
}

fn drop_target_name(self_ty: &TypeExpr) -> String {
    format!("{}.drop", type_expr_display_lossy(self_ty))
}

fn impl_target_type_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        _ => None,
    }
}

fn drop_name_span(span: ByteSpan) -> ByteSpan {
    ByteSpan::new(span.source, span.start, span.start + "drop".len())
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
