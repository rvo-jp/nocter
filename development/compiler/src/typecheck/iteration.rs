//! Protocol resolution and semantic planning for collection `for` statements.

use super::copyability::non_copy_owned_type_kind_in_environment;
use super::expressions::expression_type;
use super::interface_bounds::{conformed_interface_types, interface_symbols_for_constrained_type};
use super::interface_methods::conformance_method_for_interface;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SequenceSpreadMode {
    Copy,
    Readonly,
    Move,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceSpreadResolution {
    pub(super) mode: SequenceSpreadMode,
    pub(super) iteration: CollectionIterationResolution,
    pub(super) exact_size: IterationMethodResolution,
    pub(super) pack_item_type: Type,
}

pub(super) fn resolve_sequence_spread(
    spread: &crate::ast::UnaryExpr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Result<SequenceSpreadResolution, CollectionIterationError> {
    debug_assert_eq!(spread.operator, UnaryOperator::Spread);
    let runtime = resolved
        .trusted_declarations
        .iteration_runtime()
        .ok_or(CollectionIterationError::RuntimeUnavailable)?;
    let (mode, iteration) = match spread.operand.without_groups() {
        Expr::Borrow(borrow) if borrow.is_readwrite => {
            return Err(CollectionIterationError::MutableIterationDeferred);
        }
        Expr::Borrow(borrow) => (
            SequenceSpreadMode::Readonly,
            resolve_converted_iteration(
                CollectionIterationSourceMode::ReadonlyConversion,
                &borrow.expression,
                &runtime.readonly_conversion,
                runtime,
                resolved,
                environment,
            )?,
        ),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let source_type = expression_type(&unary.operand, resolved, environment);
            let iteration = if let Some(iterator_interface) =
                conformed_protocol_type(&source_type, &runtime.iterator, resolved, environment)
            {
                resolve_direct_iteration(
                    source_type,
                    iterator_interface,
                    runtime,
                    resolved,
                    environment,
                )?
            } else {
                resolve_converted_iteration(
                    CollectionIterationSourceMode::OwnedConversion,
                    &unary.operand,
                    &runtime.owned_conversion,
                    runtime,
                    resolved,
                    environment,
                )?
            };
            (SequenceSpreadMode::Move, iteration)
        }
        expression => (
            SequenceSpreadMode::Copy,
            resolve_converted_iteration(
                CollectionIterationSourceMode::ReadonlyConversion,
                expression,
                &runtime.readonly_conversion,
                runtime,
                resolved,
                environment,
            )?,
        ),
    };
    let _exact_interface = conformed_protocol_type(
        &iteration.iterator_type,
        &runtime.exact_size,
        resolved,
        environment,
    )
    .ok_or_else(|| CollectionIterationError::MissingExactSize(iteration.iterator_type.clone()))?;
    let pack_item_type = match mode {
        SequenceSpreadMode::Copy => {
            readonly_referent_type(&iteration.item_type).ok_or_else(|| {
                CollectionIterationError::CopyRequiresReadonlyItem(iteration.item_type.clone())
            })?
        }
        SequenceSpreadMode::Readonly | SequenceSpreadMode::Move => iteration.item_type.clone(),
    };
    if mode == SequenceSpreadMode::Copy
        && non_copy_owned_type_kind_in_environment(&pack_item_type, resolved, environment).is_some()
    {
        return Err(CollectionIterationError::CopyRequiresCopy(pack_item_type));
    }
    Ok(SequenceSpreadResolution {
        mode,
        exact_size: resolve_concrete_method(
            &iteration.iterator_type,
            &runtime.exact_size,
            resolved,
            environment,
        )?,
        iteration,
        pack_item_type,
    })
}

fn readonly_referent_type(item: &Type) -> Option<Type> {
    match item {
        Type::Borrow {
            is_readwrite: false,
            inner,
        } => Some(inner.as_ref().clone()),
        Type::Str => Some(Type::StrData),
        _ => None,
    }
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
                conformed_protocol_type(&source_type, &runtime.iterator, resolved, environment)
            {
                resolve_direct_iteration(
                    source_type,
                    iterator_interface,
                    runtime,
                    resolved,
                    environment,
                )
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
                conformed_protocol_type(&source_type, &runtime.iterator, resolved, environment)
            else {
                return Err(CollectionIterationError::AmbiguousCollection(source_type));
            };
            resolve_direct_iteration(
                source_type,
                iterator_interface,
                runtime,
                resolved,
                environment,
            )
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
    let _conversion_interface =
        conformed_protocol_type(&source_type, conversion_protocol, resolved, environment)
            .ok_or_else(|| CollectionIterationError::MissingConversion(source_type.clone()))?;
    let iterator_type = protocol_associated_type(&source_type, conversion_protocol, resolved)
        .ok_or(CollectionIterationError::MalformedConformance)?;
    let _iterator_interface =
        conformed_protocol_type(&iterator_type, &runtime.iterator, resolved, environment)
            .ok_or_else(|| CollectionIterationError::MissingIterator(iterator_type.clone()))?;
    let item_type = protocol_associated_type(&iterator_type, &runtime.iterator, resolved)
        .ok_or(CollectionIterationError::MalformedConformance)?;

    Ok(CollectionIterationResolution {
        source_mode,
        source_type: source_type.clone(),
        iterator_type: iterator_type.clone(),
        item_type,
        conversion: Some(resolve_concrete_method(
            &source_type,
            conversion_protocol,
            resolved,
            environment,
        )?),
        step: resolve_concrete_method(&iterator_type, &runtime.iterator, resolved, environment)?,
    })
}

fn resolve_direct_iteration(
    source_type: Type,
    _iterator_interface: Type,
    runtime: &IterationRuntime,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Result<CollectionIterationResolution, CollectionIterationError> {
    let item_type = protocol_associated_type(&source_type, &runtime.iterator, resolved)
        .ok_or(CollectionIterationError::MalformedConformance)?;
    Ok(CollectionIterationResolution {
        source_mode: CollectionIterationSourceMode::Direct,
        source_type: source_type.clone(),
        iterator_type: source_type.clone(),
        item_type,
        conversion: None,
        step: resolve_concrete_method(&source_type, &runtime.iterator, resolved, environment)?,
    })
}

fn protocol_associated_type(
    actual: &Type,
    protocol: &IterationProtocol,
    resolved: &ResolveOutput,
) -> Option<Type> {
    let associated = protocol.associated_type.as_ref()?;
    Some(super::associated_types::normalize_projection_for_interface(
        actual.clone(),
        &protocol.interface_canonical_name,
        associated.declaration,
        &associated.name,
        resolved,
    ))
}

fn conformed_protocol_type(
    actual: &Type,
    protocol: &IterationProtocol,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<Type> {
    if matches!(actual, Type::Parameter(_) | Type::Projection { .. }) {
        return interface_symbols_for_constrained_type(actual, environment, resolved)
            .into_iter()
            .map(|(_, bound)| bound)
            .find(|bound| protocol_type_matches(bound, protocol, resolved));
    }
    conformed_interface_types(actual, resolved)
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
    environment: &TypeEnvironment,
) -> Result<IterationMethodResolution, CollectionIterationError> {
    if matches!(receiver_type, Type::Parameter(_) | Type::Projection { .. }) {
        let method = interface_symbols_for_constrained_type(receiver_type, environment, resolved)
            .into_iter()
            .find(|(symbol, _)| symbol.canonical_name == protocol.interface_canonical_name)
            .and_then(|(symbol, _)| {
                symbol
                    .methods
                    .iter()
                    .find(|method| method.name == protocol.method_name)
            })
            .ok_or(CollectionIterationError::MalformedConformance)?;
        return Ok(iteration_method(receiver_type, method));
    }
    let method = conformance_method_for_interface(
        receiver_type,
        &protocol.interface_canonical_name,
        &protocol.method_name,
        resolved,
    )
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
    MissingExactSize(Type),
    CopyRequiresReadonlyItem(Type),
    CopyRequiresCopy(Type),
    MalformedConformance,
}

pub(super) fn collection_iteration_diagnostic(
    sources: &SourceMap,
    statement: &CollectionForStmt,
    error: CollectionIterationError,
) -> Diagnostic {
    iteration_diagnostic(sources, statement.source.span(), "E0448", error)
}

pub(super) fn sequence_spread_diagnostic(
    sources: &SourceMap,
    spread: &crate::ast::UnaryExpr,
    error: CollectionIterationError,
) -> Diagnostic {
    iteration_diagnostic(sources, spread.span, "E0524", error)
}

fn iteration_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    code: &'static str,
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
                "type `{}` does not conform to the selected collection iteration protocol",
                actual.display()
            ),
            "add the matching explicit standard iteration interface conformance".to_string(),
        ),
        CollectionIterationError::MissingIterator(actual) => (
            format!(
                "collection conversion produces `{}`, which does not conform to the iterator protocol",
                actual.display()
            ),
            "make the concrete conversion result explicitly conform to the trusted iterator interface"
                .to_string(),
        ),
        CollectionIterationError::MissingExactSize(actual) => (
            format!(
                "spread iterator `{}` does not provide an exact remaining element count",
                actual.display()
            ),
            "conform to the validated `ExactSizeIterator` interface; Nocter does not buffer unknown-size spread input"
                .to_string(),
        ),
        CollectionIterationError::CopyRequiresReadonlyItem(actual) => (
            format!("copy spread iterator yields `{}`, not a readonly reference", actual.display()),
            "use `...move source` for an owning iterator or make readonly iteration yield `&T`"
                .to_string(),
        ),
        CollectionIterationError::CopyRequiresCopy(actual) => (
            format!("copy spread element `{}` is move-only", actual.display()),
            "use `...move source` to transfer its elements explicitly".to_string(),
        ),
        CollectionIterationError::MalformedConformance => (
            "collection iteration conformance does not match its validated protocol shape".to_string(),
            "fix the interface conformance before using it in a collection loop".to_string(),
        ),
    };
    let mut diagnostic = Diagnostic::error(code, message);
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(help);
    diagnostic
}
