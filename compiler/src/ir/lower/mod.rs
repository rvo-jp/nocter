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
use crate::ast::{FunctionDecl, Item, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::Type;
use crate::resolve::ResolveOutput;
use crate::source::SourceId;
use context::{FunctionNames, FunctionSignature, FunctionSignatures};
use imported_calls::imported_call_diagnostics;
use reachability::reachable_call_targets;
use std::collections::{HashMap, HashSet};

pub(crate) fn lower_executable_with_entry(
    analysis: &CompileUnitAnalysis,
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
    let mut functions = vec![entry::lower_entry_function(
        entry,
        function_signatures.clone(),
        function_names.clone(),
        root.ast.span.source,
        &root.resolved,
    )?];
    lower_reachable_functions(
        &mut functions,
        &function_index,
        &function_signatures,
        &function_names,
        entry_name,
        root.ast.span.source,
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
        let diagnostics =
            imported_call_diagnostics(function.declaration, root_source, function.resolved);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        let function = functions::lower_function(
            function.declaration,
            target,
            function_signatures.clone(),
            function_names.clone(),
            root_source,
            function.resolved,
        )?;
        queue.extend(reachable_call_targets(&function));
        lowered.push(function);
    }

    Ok(())
}

struct FunctionIndex<'a> {
    definitions: HashMap<CallTarget, IndexedFunction<'a>>,
}

struct IndexedFunction<'a> {
    declaration: &'a FunctionDecl,
    resolved: &'a ResolveOutput,
}

impl<'a> FunctionIndex<'a> {
    fn new(analysis: &'a CompileUnitAnalysis, root_source: SourceId) -> Self {
        let mut definitions = HashMap::new();
        for file in &analysis.files {
            for item in &file.ast.items {
                let Item::Function(function) = item else {
                    continue;
                };
                let target = if file.ast.span.source == root_source {
                    CallTarget::same_file(function.name.clone())
                } else {
                    CallTarget::imported(file.ast.span.source, function.name.clone())
                };
                definitions.insert(target, IndexedFunction::new(function, file));
            }
        }
        Self { definitions }
    }

    fn definition(&self, target: &CallTarget) -> Option<&IndexedFunction<'a>> {
        self.definitions.get(target)
    }

    fn signatures(&self) -> FunctionSignatures {
        FunctionSignatures::from_call_targets(
            self.definitions
                .iter()
                .filter_map(|(target, function)| {
                    lower_signature_return_type(
                        &function.declaration.return_type,
                        function.resolved,
                    )
                    .map(|return_type| {
                        let parameter_types = function
                            .declaration
                            .parameters
                            .parameters
                            .iter()
                            .map(|parameter| {
                                lower_signature_parameter_type(&parameter.ty, function.resolved)
                            })
                            .collect::<Option<Vec<_>>>();

                        (
                            target.clone(),
                            FunctionSignature {
                                return_type,
                                parameter_types,
                            },
                        )
                    })
                })
                .collect(),
        )
    }

    fn names(&self) -> FunctionNames {
        FunctionNames::from_declarations(
            self.definitions
                .values()
                .map(|function| {
                    (
                        function.declaration.name_span,
                        function.declaration.name.clone(),
                    )
                })
                .collect(),
        )
    }
}

impl<'a> IndexedFunction<'a> {
    fn new(declaration: &'a FunctionDecl, file: &'a FileAnalysis) -> Self {
        Self {
            declaration,
            resolved: &file.resolved,
        }
    }
}

fn describe_call_target(target: &CallTarget) -> String {
    match target {
        CallTarget::SameFile(name) => name.clone(),
        CallTarget::Imported { source, name } => {
            format!("{} from source {}", name, source.raw())
        }
    }
}
