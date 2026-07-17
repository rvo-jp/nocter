mod aggregates;
mod bindings;
mod context;
mod control_flow;
mod entry;
mod errors;
mod expressions;
mod functions;
mod imported_calls;
mod literals;
mod reachability;
mod types;

#[cfg(test)]
mod tests;

use super::{CallTarget, Function, IrModule};
use crate::abi::{
    ReturnPassing, function_parameter_abi_word_count_from_signature,
    function_success_return_passing_from_signature,
};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    DropDecl, FunctionDecl, ImplMember, Item, MethodDecl, Parameter, Stmt, TypeExpr, TypeReference,
};
use crate::diagnostics::Diagnostic;
use crate::ir::Type;
use crate::resolve::{
    FunctionSignature as ResolvedFunctionSignature, ParameterSignature, ResolveOutput,
    drop_function_name,
};
use crate::source::{ByteSpan, SourceId, SourceMap};
use context::{ErrorPayloads, FunctionNames, FunctionSignature, FunctionSignatures};
use imported_calls::imported_call_diagnostics;
use reachability::reachable_call_targets;
use std::collections::{HashMap, HashSet};
use types::{parameter_type_from_type_expr, return_type_from_type_expr};

pub(crate) fn lower_executable_with_entry(
    analysis: &CompileUnitAnalysis,
    sources: &SourceMap,
    entry_name: &str,
) -> Result<IrModule, Vec<Diagnostic>> {
    let Some(root) = analysis.root_file() else {
        return Err(vec![Diagnostic::error(
            "E8000",
            "IR lowering requires a root source file",
        )]);
    };

    let Some(entry) = root.ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == entry_name => Some(function),
        _ => None,
    }) else {
        return Err(vec![
            Diagnostic::error(
                "E8000",
                format!("IR lowering requires entry function `{entry_name}`"),
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
    let mut functions = vec![
        entry::lower_entry_function(
            entry,
            sources,
            function_signatures.clone(),
            function_names.clone(),
            root.ast.span.source,
            &root.resolved,
            &root.typecheck_facts,
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
        entry_name,
        root.ast.span.source,
        sources,
    )?;

    Ok(IrModule::new(functions))
}

fn lower_signature_return_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    return_type_from_type_expr(ty, resolved)
}

fn lower_signature_parameter_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    parameter_type_from_type_expr(ty, resolved)
}

fn lower_reachable_functions(
    lowered: &mut Vec<Function>,
    function_index: &FunctionIndex<'_>,
    function_signatures: &FunctionSignatures,
    function_names: &FunctionNames,
    error_payloads: &ErrorPayloads,
    entry_name: &str,
    root_source: SourceId,
    sources: &SourceMap,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = HashSet::from([CallTarget::same_file(entry_name)]);
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
            root_source,
        )?;
        queue.extend(reachable_call_targets(&function));
        lowered.push(function);
    }

    Ok(())
}

struct FunctionIndex<'a> {
    definitions: HashMap<CallTarget, IndexedCallable<'a>>,
}

struct IndexedCallable<'a> {
    declaration: IndexedDeclaration<'a>,
    resolved: &'a ResolveOutput,
    typecheck_facts: &'a crate::typecheck::TypecheckFacts,
}

enum IndexedDeclaration<'a> {
    Function(&'a FunctionDecl),
    Drop {
        declaration: &'a DropDecl,
        self_ty: &'a TypeExpr,
        name: String,
    },
    Method {
        declaration: &'a MethodDecl,
        self_ty: &'a TypeExpr,
        name: String,
    },
}

impl<'a> FunctionIndex<'a> {
    fn new(analysis: &'a CompileUnitAnalysis, root_source: SourceId) -> Self {
        let mut definitions = HashMap::new();
        for file in &analysis.files {
            for item in &file.ast.items {
                match item {
                    Item::Function(function) => {
                        let target = call_target_for_source(
                            file.ast.span.source,
                            root_source,
                            function.name.clone(),
                        );
                        definitions.insert(target, IndexedCallable::new_function(function, file));
                    }
                    Item::Impl(impl_) if impl_.trait_ty.is_none() => {
                        let Some(type_name) = impl_target_type_name(&impl_.target_ty) else {
                            continue;
                        };
                        for member in &impl_.members {
                            match member {
                                ImplMember::Drop(drop_) => {
                                    let name = drop_function_name(type_name);
                                    let target = call_target_for_source(
                                        file.ast.span.source,
                                        root_source,
                                        name.clone(),
                                    );
                                    definitions.insert(
                                        target,
                                        IndexedCallable::new_drop(
                                            drop_,
                                            &impl_.target_ty,
                                            name,
                                            file,
                                        ),
                                    );
                                }
                                ImplMember::Method(method) if method.body.is_some() => {
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
                                            &impl_.target_ty,
                                            name,
                                            file,
                                        ),
                                    );
                                }
                                ImplMember::Method(_) => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Self { definitions }
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
                        .signature()
                        .map(|signature| (target.clone(), signature))
                })
                .collect(),
        )
    }

    fn names(&self) -> FunctionNames {
        FunctionNames::from_declarations(
            self.definitions
                .values()
                .filter_map(|function| function.name_declaration().map(|(span, name)| (span, name)))
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
            declaration: IndexedDeclaration::Function(declaration),
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
        }
    }

    fn new_drop(
        declaration: &'a DropDecl,
        self_ty: &'a TypeExpr,
        name: String,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            declaration: IndexedDeclaration::Drop {
                declaration,
                self_ty,
                name,
            },
            resolved: &file.resolved,
            typecheck_facts: &file.typecheck_facts,
        }
    }

    fn new_method(
        declaration: &'a MethodDecl,
        self_ty: &'a TypeExpr,
        name: String,
        file: &'a FileAnalysis,
    ) -> Self {
        Self {
            declaration: IndexedDeclaration::Method {
                declaration,
                self_ty,
                name,
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
            IndexedDeclaration::Function(function) => {
                imported_call_diagnostics(sources, function, root_source, self.resolved)
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
        }
    }

    fn static_error_payload(&self, root_source: SourceId) -> Option<errors::ErrorPayload> {
        let IndexedDeclaration::Function(function) = &self.declaration else {
            return None;
        };
        let [Stmt::Return(statement)] = function.body.statements.as_slice() else {
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
        root_source: SourceId,
    ) -> Result<Function, Vec<Diagnostic>> {
        let span = self.declaration.span();
        match &self.declaration {
            IndexedDeclaration::Function(function) => functions::lower_function(
                function,
                sources,
                target,
                function_signatures,
                function_names,
                root_source,
                self.resolved,
                self.typecheck_facts,
                error_payloads,
            ),
            IndexedDeclaration::Drop {
                declaration,
                self_ty,
                name,
            } => functions::lower_drop_function(
                declaration,
                self_ty,
                name.clone(),
                sources,
                target,
                function_signatures,
                function_names,
                root_source,
                self.resolved,
                self.typecheck_facts,
                error_payloads,
            ),
            IndexedDeclaration::Method {
                declaration,
                self_ty,
                name,
            } => functions::lower_method_function(
                declaration,
                self_ty,
                name.clone(),
                sources,
                target,
                function_signatures,
                function_names,
                root_source,
                self.resolved,
                self.typecheck_facts,
                error_payloads,
            ),
        }
        .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, span))
    }

    fn signature(&self) -> Option<FunctionSignature> {
        match &self.declaration {
            IndexedDeclaration::Function(function) => {
                let resolved_signature = resolved_function_signature(
                    &function.parameters.parameters,
                    function.return_type.clone(),
                );
                lower_signature_return_type(&function.return_type, self.resolved).map(
                    |return_type| {
                        let parameter_types = function
                            .parameters
                            .parameters
                            .iter()
                            .map(|parameter| {
                                lower_signature_parameter_type(&parameter.ty, self.resolved)
                            })
                            .collect::<Option<Vec<_>>>();

                        FunctionSignature {
                            return_type,
                            parameter_types,
                            parameter_abi_word_count: parameter_abi_word_count(
                                &resolved_signature,
                                self.resolved,
                            ),
                            success_return_passing: success_return_passing(
                                &resolved_signature,
                                self.resolved,
                            ),
                        }
                    },
                )
            }
            IndexedDeclaration::Drop {
                declaration,
                self_ty,
                ..
            } => {
                let parameter_ty =
                    functions::type_expr_with_self_type(&declaration.binding.ty, self_ty);
                let parameter_type = lower_signature_parameter_type(&parameter_ty, self.resolved)?;
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
                    ),
                    success_return_passing: success_return_passing(
                        &resolved_signature,
                        self.resolved,
                    ),
                })
            }
            IndexedDeclaration::Method {
                declaration,
                self_ty,
                ..
            } => {
                let parameters = method_parameters(declaration, self_ty);
                let return_type =
                    functions::type_expr_with_self_type(&declaration.return_type, self_ty);
                let resolved_signature =
                    resolved_function_signature(&parameters, return_type.clone());
                lower_signature_return_type(&return_type, self.resolved).map(|return_type| {
                    let parameter_types = parameters
                        .iter()
                        .map(|parameter| {
                            lower_signature_parameter_type(&parameter.ty, self.resolved)
                        })
                        .collect::<Option<Vec<_>>>();

                    FunctionSignature {
                        return_type,
                        parameter_types,
                        parameter_abi_word_count: parameter_abi_word_count(
                            &resolved_signature,
                            self.resolved,
                        ),
                        success_return_passing: success_return_passing(
                            &resolved_signature,
                            self.resolved,
                        ),
                    }
                })
            }
        }
    }

    fn name_declaration(&self) -> Option<(crate::source::ByteSpan, String)> {
        match &self.declaration {
            IndexedDeclaration::Function(function) => {
                Some((function.name_span, function.name.clone()))
            }
            IndexedDeclaration::Drop {
                declaration, name, ..
            } => Some((drop_name_span(declaration.span), name.clone())),
            IndexedDeclaration::Method {
                declaration, name, ..
            } => Some((declaration.name_span, name.clone())),
        }
    }
}

fn method_parameters(method: &MethodDecl, self_ty: &TypeExpr) -> Vec<Parameter> {
    let mut parameters = Vec::with_capacity(method.parameters.parameters.len() + 1);
    parameters.push(Parameter {
        span: method.receiver.span,
        name: method.receiver.name.clone(),
        name_span: method.receiver.name_span,
        ty: functions::type_expr_with_self_type(&method.receiver.ty, self_ty),
    });
    parameters.extend(method.parameters.parameters.iter().cloned());
    parameters
}

fn resolved_function_signature(
    parameters: &[Parameter],
    return_type: TypeExpr,
) -> ResolvedFunctionSignature {
    ResolvedFunctionSignature {
        parameters: parameters
            .iter()
            .map(|parameter| ParameterSignature {
                name: parameter.name.clone(),
                name_span: parameter.name_span,
                ty: parameter.ty.clone(),
            })
            .collect(),
        return_type,
    }
}

fn parameter_abi_word_count(
    signature: &ResolvedFunctionSignature,
    resolved: &ResolveOutput,
) -> Option<usize> {
    function_parameter_abi_word_count_from_signature(signature, resolved).ok()
}

fn success_return_passing(
    signature: &ResolvedFunctionSignature,
    resolved: &ResolveOutput,
) -> Option<ReturnPassing> {
    function_success_return_passing_from_signature(signature, resolved).ok()
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
            IndexedDeclaration::Function(function) => function.span,
            IndexedDeclaration::Drop { declaration, .. } => declaration.span,
            IndexedDeclaration::Method { declaration, .. } => declaration.span,
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

fn method_target_name(type_name: &str, method_name: &str) -> String {
    format!("{type_name}.{method_name}")
}

fn impl_target_type_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
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
