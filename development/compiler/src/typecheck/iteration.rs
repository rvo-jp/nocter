//! Protocol resolution and semantic planning for collection `for` statements.

use super::calls::method_applies_to_receiver;
use super::expressions::expression_type;
use super::interface_bounds::implemented_interface_types;
use super::model::{Type, TypeEnvironment};
use crate::ast::{CollectionForStmt, Expr, MethodReceiverMode, UnaryOperator};
use crate::diagnostics::Diagnostic;
use crate::resolve::{MethodSignature, ResolveOutput};
use crate::semantics::{IterationProtocol, IterationRuntime};
use crate::source::{ByteSpan, SourceMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CollectionIterationSourceMode {
    Direct,
    ReadonlyConversion,
    OwnedConversion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct IterationMethodResolution {
    pub(super) declaration: ByteSpan,
    pub(super) method_name: String,
    pub(super) target_name: String,
    pub(super) receiver_mode: MethodReceiverMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CollectionIterationResolution {
    pub(super) source_mode: CollectionIterationSourceMode,
    pub(super) source_type: Type,
    pub(super) iterator_type: Type,
    pub(super) item_type: Type,
    pub(super) conversion: Option<IterationMethodResolution>,
    pub(super) step: IterationMethodResolution,
}

pub(super) fn resolve_collection_iteration(
    statement: &CollectionForStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Result<CollectionIterationResolution, CollectionIterationError> {
    let runtime = resolved
        .trusted_declarations
        .iteration_runtime()
        .ok_or(CollectionIterationError::RuntimeUnavailable)?;

    match statement.source.without_groups() {
        Expr::Borrow(borrow) if borrow.is_readwrite => {
            Err(CollectionIterationError::MutableIterationDeferred)
        }
        Expr::Borrow(borrow) => resolve_converted_iteration(
            CollectionIterationSourceMode::ReadonlyConversion,
            &borrow.expression,
            &runtime.readonly_conversion,
            runtime,
            resolved,
            environment,
        ),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let source_type = expression_type(&unary.operand, resolved, environment);
            if let Some(iterator_interface) =
                implemented_protocol_type(&source_type, &runtime.iterator, resolved)
            {
                resolve_direct_iteration(source_type, iterator_interface, runtime, resolved)
            } else {
                resolve_converted_iteration(
                    CollectionIterationSourceMode::OwnedConversion,
                    &unary.operand,
                    &runtime.owned_conversion,
                    runtime,
                    resolved,
                    environment,
                )
            }
        }
        expression => {
            let source_type = expression_type(expression, resolved, environment);
            let Some(iterator_interface) =
                implemented_protocol_type(&source_type, &runtime.iterator, resolved)
            else {
                return Err(CollectionIterationError::AmbiguousCollection(source_type));
            };
            resolve_direct_iteration(source_type, iterator_interface, runtime, resolved)
        }
    }
}

fn resolve_converted_iteration(
    source_mode: CollectionIterationSourceMode,
    expression: &Expr,
    conversion_protocol: &IterationProtocol,
    runtime: &IterationRuntime,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Result<CollectionIterationResolution, CollectionIterationError> {
    let source_type = expression_type(expression, resolved, environment);
    if source_type.is_unknown_or_unresolved() {
        return Err(CollectionIterationError::UnresolvedSource);
    }
    let conversion_interface =
        implemented_protocol_type(&source_type, conversion_protocol, resolved)
            .ok_or_else(|| CollectionIterationError::MissingConversion(source_type.clone()))?;
    let Type::Generic { arguments, .. } = conversion_interface else {
        return Err(CollectionIterationError::MalformedConformance);
    };
    let [item_type, iterator_type] = arguments.as_slice() else {
        return Err(CollectionIterationError::MalformedConformance);
    };
    let iterator_interface = implemented_protocol_type(iterator_type, &runtime.iterator, resolved)
        .ok_or_else(|| CollectionIterationError::MissingIterator(iterator_type.clone()))?;
    let iterator_item = protocol_item_type(&iterator_interface)
        .ok_or(CollectionIterationError::MalformedConformance)?;
    if iterator_item != item_type {
        return Err(CollectionIterationError::MismatchedItem {
            conversion: item_type.clone(),
            iterator: iterator_item.clone(),
        });
    }

    Ok(CollectionIterationResolution {
        source_mode,
        source_type: source_type.clone(),
        iterator_type: iterator_type.clone(),
        item_type: item_type.clone(),
        conversion: Some(resolve_concrete_method(
            &source_type,
            conversion_protocol,
            resolved,
        )?),
        step: resolve_concrete_method(iterator_type, &runtime.iterator, resolved)?,
    })
}

fn resolve_direct_iteration(
    source_type: Type,
    iterator_interface: Type,
    runtime: &IterationRuntime,
    resolved: &ResolveOutput,
) -> Result<CollectionIterationResolution, CollectionIterationError> {
    let item_type = protocol_item_type(&iterator_interface)
        .ok_or(CollectionIterationError::MalformedConformance)?
        .clone();
    Ok(CollectionIterationResolution {
        source_mode: CollectionIterationSourceMode::Direct,
        source_type: source_type.clone(),
        iterator_type: source_type.clone(),
        item_type,
        conversion: None,
        step: resolve_concrete_method(&source_type, &runtime.iterator, resolved)?,
    })
}

fn protocol_item_type(interface: &Type) -> Option<&Type> {
    let Type::Generic { arguments, .. } = interface else {
        return None;
    };
    let [item] = arguments.as_slice() else {
        return None;
    };
    Some(item)
}

fn implemented_protocol_type(
    actual: &Type,
    protocol: &IterationProtocol,
    resolved: &ResolveOutput,
) -> Option<Type> {
    implemented_interface_types(actual, resolved)
        .into_iter()
        .find(|implemented| protocol_type_matches(implemented, protocol, resolved))
}

fn protocol_type_matches(
    implemented: &Type,
    protocol: &IterationProtocol,
    _resolved: &ResolveOutput,
) -> bool {
    implemented.nominal_name() == Some(protocol.interface_canonical_name.as_str())
}

fn resolve_concrete_method(
    receiver_type: &Type,
    protocol: &IterationProtocol,
    resolved: &ResolveOutput,
) -> Result<IterationMethodResolution, CollectionIterationError> {
    let symbol = receiver_type
        .nominal_name()
        .and_then(|name| resolved.type_symbol_by_canonical_name(name))
        .ok_or(CollectionIterationError::MalformedConformance)?;
    let method = symbol
        .methods
        .iter()
        .find(|method| {
            method.name == protocol.method_name
                && method_applies_to_receiver(method, receiver_type, resolved)
        })
        .ok_or(CollectionIterationError::MalformedConformance)?;
    Ok(iteration_method(receiver_type, method))
}

fn iteration_method(receiver_type: &Type, method: &MethodSignature) -> IterationMethodResolution {
    IterationMethodResolution {
        declaration: method.name_span,
        method_name: method.name.clone(),
        target_name: format!("{}.{}", receiver_type.display(), method.name),
        receiver_mode: method.receiver.mode,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CollectionIterationError {
    RuntimeUnavailable,
    MutableIterationDeferred,
    UnresolvedSource,
    AmbiguousCollection(Type),
    MissingConversion(Type),
    MissingIterator(Type),
    MismatchedItem { conversion: Type, iterator: Type },
    MalformedConformance,
}

pub(super) fn collection_iteration_diagnostic(
    sources: &SourceMap,
    statement: &CollectionForStmt,
    error: CollectionIterationError,
) -> Diagnostic {
    let (message, help) = match error {
        CollectionIterationError::RuntimeUnavailable => (
            "collection iteration protocols are unavailable in the active Nocter home".to_string(),
            "use a complete Nocter home containing the validated iteration interfaces".to_string(),
        ),
        CollectionIterationError::MutableIterationDeferred => (
            "mutable collection iteration is not part of v0.3.0 Phase 7".to_string(),
            "use `&collection` for readonly iteration or `move collection` for consuming iteration"
                .to_string(),
        ),
        CollectionIterationError::UnresolvedSource => (
            "the collection iteration source type could not be resolved".to_string(),
            "resolve the source expression before iterating it".to_string(),
        ),
        CollectionIterationError::AmbiguousCollection(actual) => (
            format!(
                "`{}` is not a direct iterator; collection iteration requires an explicit ownership mode",
                actual.display()
            ),
            "write `&collection` for readonly iteration or `move collection` for consuming iteration"
                .to_string(),
        ),
        CollectionIterationError::MissingConversion(actual) => (
            format!(
                "type `{}` does not implement the selected collection iteration protocol",
                actual.display()
            ),
            "add the matching explicit standard iteration interface conformance".to_string(),
        ),
        CollectionIterationError::MissingIterator(actual) => (
            format!(
                "collection conversion produces `{}`, which does not implement the iterator protocol",
                actual.display()
            ),
            "make the concrete conversion result explicitly conform to the trusted iterator interface"
                .to_string(),
        ),
        CollectionIterationError::MismatchedItem {
            conversion,
            iterator,
        } => (
            format!(
                "collection conversion declares item `{}`, but its iterator yields `{}`",
                conversion.display(),
                iterator.display()
            ),
            "make the conversion and iterator conformances use the same item type".to_string(),
        ),
        CollectionIterationError::MalformedConformance => (
            "collection iteration conformance does not match its validated protocol shape".to_string(),
            "fix the interface implementation before using it in a collection loop".to_string(),
        ),
    };
    let mut diagnostic = Diagnostic::error("E0448", message);
    diagnostic.primary_span = sources
        .span_to_json(statement.source.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(help);
    diagnostic
}
