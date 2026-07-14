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

#[cfg(test)]
mod tests;

use super::{CallTarget, Function, IrModule};
use crate::abi::{AbiType, AbiValue, ValueClassification, abi_value_from_type_expr};
use crate::analysis::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{DropDecl, FunctionDecl, ImplMember, Item, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::Type;
use crate::resolve::{ResolveOutput, drop_function_name};
use crate::source::{ByteSpan, SourceId, SourceMap};
use context::{FunctionNames, FunctionSignature, FunctionSignatures};
use imported_calls::imported_call_diagnostics;
use reachability::reachable_call_targets;
use std::collections::{HashMap, HashSet};

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
        return Err(vec![Diagnostic::error(
            "E8000",
            format!("IR lowering requires entry function `{entry_name}`"),
        )]);
    };

    let function_index = FunctionIndex::new(analysis, root.ast.span.source);
    let diagnostics = imported_call_diagnostics(entry, root.ast.span.source, &root.resolved);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let function_signatures = function_index.signatures();
    let function_names = function_index.names();
    let mut functions = vec![
        entry::lower_entry_function(
            entry,
            sources,
            function_signatures.clone(),
            function_names.clone(),
            root.ast.span.source,
            &root.resolved,
        )
        .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, entry.span))?,
    ];
    lower_reachable_functions(
        &mut functions,
        &function_index,
        &function_signatures,
        &function_names,
        entry_name,
        root.ast.span.source,
        sources,
    )?;

    Ok(IrModule::new(functions))
}

fn lower_signature_return_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Some(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "u8" => Some(Type::U8),
        TypeExpr::Reference(reference) if reference.name == "usize" => Some(Type::Usize),
        TypeExpr::Reference(reference) if reference.name == "bool" => Some(Type::Bool),
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "str") =>
        {
            Some(Type::Str)
        }
        TypeExpr::Borrow(borrow) if is_u8_slice_data_type(&borrow.inner) => Some(Type::Slice {
            is_readwrite: borrow.is_readwrite,
        }),
        TypeExpr::Reference(reference) if reference.name == "void" => Some(Type::Void),
        TypeExpr::Reference(reference) if reference.name == "never" => Some(Type::Never),
        TypeExpr::Fallible(fallible) => lower_signature_return_type(&fallible.success, resolved)
            .map(|success| Type::Fallible(Box::new(success))),
        _ => lower_aggregate_signature_return_type(ty, resolved),
    }
}

fn lower_aggregate_signature_return_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    let value = abi_value_from_type_expr(ty, resolved).ok()?;
    aggregate_type_from_abi_value(&value)
}

fn lower_signature_parameter_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Some(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "u8" => Some(Type::U8),
        TypeExpr::Reference(reference) if reference.name == "usize" => Some(Type::Usize),
        TypeExpr::Reference(reference) if reference.name == "bool" => Some(Type::Bool),
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "str") =>
        {
            Some(Type::Str)
        }
        TypeExpr::Borrow(borrow) if is_u8_slice_data_type(&borrow.inner) => Some(Type::Slice {
            is_readwrite: borrow.is_readwrite,
        }),
        TypeExpr::Borrow(borrow) => {
            borrow_inner_type(&borrow.inner, resolved).map(|inner| Type::Borrow {
                is_readwrite: borrow.is_readwrite,
                inner: Box::new(inner),
            })
        }
        _ => lower_aggregate_signature_parameter_type(ty, resolved),
    }
}

fn lower_aggregate_signature_parameter_type(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Option<Type> {
    let value = abi_value_from_type_expr(ty, resolved).ok()?;
    aggregate_type_from_abi_value(&value)
}

fn is_u8_slice_data_type(ty: &TypeExpr) -> bool {
    matches!(
        ty,
        TypeExpr::View(view)
            if !view.is_readwrite
                && matches!(view.element.as_ref(), TypeExpr::Reference(reference) if reference.name == "u8")
    )
}

fn borrow_inner_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    let scalar = match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Some(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "u8" => Some(Type::U8),
        TypeExpr::Reference(reference) if reference.name == "usize" => Some(Type::Usize),
        TypeExpr::Reference(reference) if reference.name == "bool" => Some(Type::Bool),
        _ => None,
    };
    if scalar.is_some() {
        return scalar;
    }

    let value = abi_value_from_type_expr(ty, resolved).ok()?;
    aggregate_type_from_abi_value(&value)
}

fn aggregate_type_from_abi_value(value: &AbiValue) -> Option<Type> {
    if !matches!(value.ty, AbiType::Struct(_)) {
        return None;
    }

    match value.classification {
        ValueClassification::Indirect => Some(Type::Aggregate {
            layout: value.layout,
        }),
        ValueClassification::Direct { words } => Some(Type::DirectAggregate {
            layout: value.layout,
            words,
        }),
    }
}

fn lower_reachable_functions(
    lowered: &mut Vec<Function>,
    function_index: &FunctionIndex<'_>,
    function_signatures: &FunctionSignatures,
    function_names: &FunctionNames,
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
        let diagnostics = function.imported_call_diagnostics(root_source);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let function = function.lower(
            target,
            sources,
            function_signatures.clone(),
            function_names.clone(),
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
}

enum IndexedDeclaration<'a> {
    Function(&'a FunctionDecl),
    Drop {
        declaration: &'a DropDecl,
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
                            let ImplMember::Drop(drop_) = member else {
                                continue;
                            };
                            let name = drop_function_name(type_name);
                            let target = call_target_for_source(
                                file.ast.span.source,
                                root_source,
                                name.clone(),
                            );
                            definitions.insert(
                                target,
                                IndexedCallable::new_drop(drop_, &impl_.target_ty, name, file),
                            );
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
}

impl<'a> IndexedCallable<'a> {
    fn new_function(declaration: &'a FunctionDecl, file: &'a FileAnalysis) -> Self {
        Self {
            declaration: IndexedDeclaration::Function(declaration),
            resolved: &file.resolved,
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
        }
    }

    fn imported_call_diagnostics(&self, root_source: SourceId) -> Vec<Diagnostic> {
        match &self.declaration {
            IndexedDeclaration::Function(function) => {
                imported_call_diagnostics(function, root_source, self.resolved)
            }
            IndexedDeclaration::Drop { declaration, .. } => {
                imported_calls::imported_call_diagnostics_for_block(
                    &declaration.body,
                    root_source,
                    self.resolved,
                )
            }
        }
    }

    fn lower(
        &self,
        target: CallTarget,
        sources: &SourceMap,
        function_signatures: FunctionSignatures,
        function_names: FunctionNames,
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
            ),
        }
        .map_err(|diagnostics| attach_primary_span_if_absent(diagnostics, sources, span))
    }

    fn signature(&self) -> Option<FunctionSignature> {
        match &self.declaration {
            IndexedDeclaration::Function(function) => lower_signature_return_type(
                &function.return_type,
                self.resolved,
            )
            .map(|return_type| {
                let parameter_types = function
                    .parameters
                    .parameters
                    .iter()
                    .map(|parameter| lower_signature_parameter_type(&parameter.ty, self.resolved))
                    .collect::<Option<Vec<_>>>();

                FunctionSignature {
                    return_type,
                    parameter_types,
                }
            }),
            IndexedDeclaration::Drop {
                declaration,
                self_ty,
                ..
            } => {
                let parameter_ty =
                    functions::type_expr_with_self_type(&declaration.binding.ty, self_ty);
                let parameter_type = lower_signature_parameter_type(&parameter_ty, self.resolved)?;
                Some(FunctionSignature {
                    return_type: Type::Void,
                    parameter_types: Some(vec![parameter_type]),
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
        }
    }
}

impl IndexedDeclaration<'_> {
    fn span(&self) -> ByteSpan {
        match self {
            IndexedDeclaration::Function(function) => function.span,
            IndexedDeclaration::Drop { declaration, .. } => declaration.span,
        }
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

fn call_target_for_source(source: SourceId, root_source: SourceId, name: String) -> CallTarget {
    if source == root_source {
        CallTarget::same_file(name)
    } else {
        CallTarget::imported(source, name)
    }
}

fn impl_target_type_name(ty: &TypeExpr) -> Option<&str> {
    let TypeExpr::Reference(reference) = ty else {
        return None;
    };
    Some(&reference.name)
}

fn drop_name_span(span: crate::source::ByteSpan) -> crate::source::ByteSpan {
    crate::source::ByteSpan::new(span.source, span.start, span.start + "drop".len())
}

fn describe_call_target(target: &CallTarget) -> String {
    match target {
        CallTarget::SameFile(name) => name.clone(),
        CallTarget::Imported { source, name } => {
            format!("{} from source {}", name, source.raw())
        }
    }
}
