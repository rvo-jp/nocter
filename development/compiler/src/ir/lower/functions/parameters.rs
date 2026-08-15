use super::*;
use crate::integer::IntegerType;

pub(super) fn function_parameters(
    function: &FunctionDecl,
    substitutions: &HashMap<String, TypeExpr>,
) -> Vec<Parameter> {
    crate::callable_parameters::function(&function.parameters.parameters, substitutions)
}

pub(in crate::ir::lower) fn method_parameters(
    method: &crate::ast::CallableDecl,
    self_ty: &TypeExpr,
    substitutions: &HashMap<String, TypeExpr>,
) -> Vec<Parameter> {
    crate::callable_parameters::instance(method, self_ty, substitutions)
}

pub(in crate::ir::lower) fn lower_scalar_parameters(
    function_name: &str,
    parameters: &[Parameter],
    root_source: SourceId,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    sources: &SourceMap,
) -> Result<LoweringParameterSlots, Vec<Diagnostic>> {
    let mut slots = LoweringParameterSlots::default();
    for parameter in parameters {
        match lower_scalar_parameter_kind(
            parameter,
            function_name,
            root_source,
            resolved,
            resolved_sources,
        )
        .map_err(|diagnostics| {
            attach_primary_span_if_absent(diagnostics, sources, parameter.span)
        })? {
            ScalarParameterKind::I32 => {
                slots.push_i32_parameter(parameter.name.clone());
            }
            ScalarParameterKind::U8 => {
                slots.push_u8_parameter(parameter.name.clone());
            }
            ScalarParameterKind::Usize => {
                slots.push_usize_parameter(parameter.name.clone());
            }
            ScalarParameterKind::Integer(kind) => {
                slots.push_integer_parameter(parameter.name.clone(), kind);
            }
            ScalarParameterKind::Bool => {
                slots.push_bool_parameter(parameter.name.clone());
            }
            ScalarParameterKind::Str => {
                slots.push_str_parameter(parameter.name.clone());
                slots.push_empty_abi_word();
            }
            ScalarParameterKind::Slice(info) => {
                slots.push_slice_parameter(
                    parameter.name.clone(),
                    info.element_kind,
                    info.element_type,
                );
                slots.push_empty_abi_word();
            }
            ScalarParameterKind::Error => {
                slots.push_error_parameter(parameter.name.clone());
            }
            ScalarParameterKind::Borrow => {
                let parameter_index = slots.reserve_empty_abi_words(1);
                slots.borrow_parameters.push(BorrowParameter {
                    name: parameter.name.clone(),
                    parameter_index,
                });
                slots.push_source_storage(ParameterStorage::Borrow {
                    abi_index: parameter_index,
                });
            }
            ScalarParameterKind::BorrowAggregate { .. } => {
                let parameter_index = slots.reserve_empty_abi_words(1);
                slots.aggregate_borrows.push(AggregateBorrowParameter {
                    name: parameter.name.clone(),
                    parameter_index,
                });
                slots.push_source_storage(ParameterStorage::Borrow {
                    abi_index: parameter_index,
                });
            }
            ScalarParameterKind::AggregateIndirect { layout } => {
                let parameter_index = slots.reserve_empty_abi_words(1);
                let slot_index = slots.aggregates.len();
                slots.aggregates.push(LoweringAggregateParameter {
                    name: parameter.name.clone(),
                    layout,
                    slot_index,
                    source: AggregateParameterSource::Indirect { parameter_index },
                });
                slots.push_source_storage(ParameterStorage::Aggregate {
                    slot_index,
                    layout,
                    classification: ValueClassification::Indirect,
                });
            }
            ScalarParameterKind::AggregateDirect { layout, words } => {
                let start_index = slots.reserve_empty_abi_words(words);
                let slot_index = slots.aggregates.len();
                slots.aggregates.push(LoweringAggregateParameter {
                    name: parameter.name.clone(),
                    layout,
                    slot_index,
                    source: AggregateParameterSource::Direct { start_index, words },
                });
                slots.push_source_storage(ParameterStorage::Aggregate {
                    slot_index,
                    layout,
                    classification: ValueClassification::Direct { words },
                });
            }
            ScalarParameterKind::OutcomeIndirect { storage, .. } => {
                let parameter_index = slots.reserve_empty_abi_words(1);
                let slot_index = slots.aggregates.len() + slots.outcomes.len();
                let layout = storage.layout;
                slots.outcomes.push(LoweringOutcomeParameter {
                    name: parameter.name.clone(),
                    storage,
                    slot_index,
                    source: AggregateParameterSource::Indirect { parameter_index },
                });
                slots.push_source_storage(ParameterStorage::Aggregate {
                    slot_index,
                    layout,
                    classification: ValueClassification::Indirect,
                });
            }
            ScalarParameterKind::OutcomeDirect { storage, words, .. } => {
                let start_index = slots.reserve_empty_abi_words(words);
                let slot_index = slots.aggregates.len() + slots.outcomes.len();
                let layout = storage.layout;
                slots.outcomes.push(LoweringOutcomeParameter {
                    name: parameter.name.clone(),
                    storage,
                    slot_index,
                    source: AggregateParameterSource::Direct { start_index, words },
                });
                slots.push_source_storage(ParameterStorage::Aggregate {
                    slot_index,
                    layout,
                    classification: ValueClassification::Direct { words },
                });
            }
        }
    }

    Ok(slots)
}

pub(in crate::ir::lower) fn validate_parameter_slots_match_function_abi(
    function_name: &str,
    parameters: &[Parameter],
    return_type: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    slots: &LoweringParameterSlots,
) -> Result<(), Vec<Diagnostic>> {
    let signature = resolved_function_signature(parameters, return_type.clone());
    let expected = function_parameter_abi_word_count_from_signature_with_resolver(
        &signature,
        resolved,
        |source| resolved_sources.get(&source).copied(),
    )
    .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    let actual = slots.parameter_abi_word_count();
    if actual == expected {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8007",
        format!(
            "native lowering produced {actual} parameter ABI words for function `{function_name}`, but the resolved ABI expects {expected}"
        ),
    )])
}

pub(in crate::ir::lower) fn resolved_function_signature(
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

pub(super) fn void_type_expr(span: ByteSpan) -> TypeExpr {
    TypeExpr::Reference(TypeReference {
        span,
        name: "void".to_string(),
    })
}

pub(in crate::ir::lower) fn lower_aggregate_parameter_setup(
    parameters: &LoweringParameterSlots,
) -> Vec<Instruction> {
    let mut instructions = Vec::new();
    for parameter in &parameters.aggregates {
        instructions.push(Instruction::ReserveAggregateSlot {
            slot_index: parameter.slot_index,
            layout: parameter.layout,
        });
        let source = match parameter.source {
            AggregateParameterSource::Indirect { parameter_index } => {
                AggregateLocation::Parameter(parameter_index)
            }
            AggregateParameterSource::Direct { start_index, .. } => {
                AggregateLocation::DirectParameter { start_index }
            }
        };
        instructions.push(Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(parameter.slot_index),
            source,
            layout: parameter.layout,
        });
    }
    for parameter in &parameters.outcomes {
        instructions.push(Instruction::ReserveAggregateSlot {
            slot_index: parameter.slot_index,
            layout: parameter.storage.layout,
        });
        let source = match parameter.source {
            AggregateParameterSource::Indirect { parameter_index } => {
                AggregateLocation::Parameter(parameter_index)
            }
            AggregateParameterSource::Direct { start_index, .. } => {
                AggregateLocation::DirectParameter { start_index }
            }
        };
        instructions.push(Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(parameter.slot_index),
            source,
            layout: parameter.storage.layout,
        });
    }
    instructions
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ScalarParameterKind {
    I32,
    U8,
    Usize,
    Integer(IntegerType),
    Bool,
    Str,
    Slice(SliceTypeInfo),
    Error,
    Borrow,
    BorrowAggregate {
        layout: crate::abi::ValueLayout,
    },
    AggregateIndirect {
        layout: crate::abi::ValueLayout,
    },
    AggregateDirect {
        layout: crate::abi::ValueLayout,
        words: usize,
    },
    OutcomeIndirect {
        storage: crate::outcomes::storage::OutcomeStorageLayout,
    },
    OutcomeDirect {
        storage: crate::outcomes::storage::OutcomeStorageLayout,
        words: usize,
    },
}

pub(super) fn lower_scalar_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    root_source: SourceId,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    match parameter_type_from_type_expr_with_resolver(&parameter.ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    }) {
        Some(Type::I32) => return Ok(ScalarParameterKind::I32),
        Some(Type::U8) => return Ok(ScalarParameterKind::U8),
        Some(Type::Usize) => return Ok(ScalarParameterKind::Usize),
        Some(Type::Integer(kind)) => return Ok(ScalarParameterKind::Integer(kind)),
        Some(Type::Bool) => return Ok(ScalarParameterKind::Bool),
        Some(Type::Str) => return Ok(ScalarParameterKind::Str),
        Some(Type::Slice { .. }) => {
            return Ok(ScalarParameterKind::Slice(slice_type_info_from_type_expr(
                &parameter.ty,
                resolved,
                resolved_sources,
            )));
        }
        Some(Type::Error) => return Ok(ScalarParameterKind::Error),
        _ => {}
    }

    let value = abi_value_from_type_expr_with_resolver(&parameter.ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
    .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    if matches!(value.ty, AbiType::Outcome { .. }) {
        let shape = outcome_shape_with_resolver(&parameter.ty, resolved, |source| {
            resolved_sources.get(&source).copied()
        });
        let payload = abi_value_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
            resolved_sources.get(&source).copied()
        })
        .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
        let storage = shape
            .storage_layout(payload.layout)
            .ok_or_else(|| unsupported_parameter_type_diagnostic(function_name))?;
        return Ok(match value.classification {
            crate::abi::ValueClassification::Indirect => {
                ScalarParameterKind::OutcomeIndirect { storage }
            }
            crate::abi::ValueClassification::Direct { words } => {
                ScalarParameterKind::OutcomeDirect { storage, words }
            }
        });
    }
    match &value.ty {
        AbiType::Borrow => lower_borrow_parameter_kind(
            parameter,
            function_name,
            root_source,
            resolved,
            resolved_sources,
        ),
        AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_) => {
            lower_aggregate_parameter_kind(
                parameter,
                function_name,
                root_source,
                resolved,
                resolved_sources,
                &value,
            )
        }
        _ => Err(unsupported_parameter_type_diagnostic(function_name)),
    }
}

pub(super) fn slice_element_kind_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> TypecheckSliceElementKind
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match view_element_type_from_type_expr_with_resolver(ty, resolved, resolver) {
        Some(Type::I32) => TypecheckSliceElementKind::I32,
        Some(Type::U8) => TypecheckSliceElementKind::U8,
        Some(Type::Usize) => TypecheckSliceElementKind::Usize,
        Some(Type::Integer(kind)) => TypecheckSliceElementKind::Integer(kind),
        Some(Type::Bool) => TypecheckSliceElementKind::Bool,
        Some(Type::Str) => TypecheckSliceElementKind::Str,
        _ => TypecheckSliceElementKind::Other,
    }
}

pub(super) fn slice_type_info_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> SliceTypeInfo {
    slice_type_info_from_type_expr_with_resolver(ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
}

pub(super) fn slice_type_info_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolver: F,
) -> SliceTypeInfo
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    SliceTypeInfo {
        element_kind: slice_element_kind_from_type_expr_with_resolver(ty, resolved, &resolver),
        element_type: view_element_type_expr_from_type_expr_with_resolver(ty, resolved, &resolver),
    }
}

pub(super) fn view_element_type_expr_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<TypeExpr>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(borrow) => {
            let TypeExpr::View(view) = borrow.inner.as_ref() else {
                return None;
            };
            Some(*view.element.clone())
        }
        TypeExpr::Reference(reference) => {
            let source_resolved = resolver(ty.span().source).unwrap_or(resolved);
            let symbol = source_resolved
                .type_symbol_by_reference_name(&reference.name)
                .or_else(|| resolved.type_symbol_by_reference_name(&reference.name))?;
            let target = symbol.alias_target.as_ref()?;
            let target_resolved = resolver(target.span().source).unwrap_or(source_resolved);
            view_element_type_expr_from_type_expr_with_resolver(target, target_resolved, resolver)
        }
        _ => None,
    }
}

pub(super) fn lower_borrow_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    root_source: SourceId,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    let Some(borrow) = borrow_type_from_type_expr(&parameter.ty, resolved) else {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    };
    match borrow_inner_type_with_resolver(&borrow.inner, resolved, |source| {
        resolved_sources.get(&source).copied()
    }) {
        Some(Type::Aggregate { .. } | Type::DirectAggregate { .. }) => {
            lower_aggregate_borrow_parameter_kind(
                parameter,
                function_name,
                root_source,
                resolved,
                resolved_sources,
            )
        }
        Some(_) => Ok(ScalarParameterKind::Borrow),
        None => Err(unsupported_parameter_type_diagnostic(function_name)),
    }
}

pub(super) fn lower_aggregate_borrow_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    _root_source: SourceId,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    let Some(borrow) = borrow_type_from_type_expr(&parameter.ty, resolved) else {
        unreachable!("aggregate borrow parameter lowering requires a borrow type");
    };
    let value = abi_value_from_type_expr_with_resolver(&borrow.inner, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
    .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    if !matches!(value.ty, AbiType::Struct(_)) {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    }
    Ok(ScalarParameterKind::BorrowAggregate {
        layout: value.layout,
    })
}

pub(super) fn lower_aggregate_parameter_kind(
    _parameter: &Parameter,
    function_name: &str,
    _root_source: SourceId,
    _resolved: &ResolveOutput,
    _resolved_sources: &ResolvedSources<'_>,
    value: &AbiValue,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    if !matches!(
        value.ty,
        AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_)
    ) {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    }
    match value.classification {
        ValueClassification::Indirect => Ok(ScalarParameterKind::AggregateIndirect {
            layout: value.layout,
        }),
        ValueClassification::Direct { words } => Ok(ScalarParameterKind::AggregateDirect {
            layout: value.layout,
            words,
        }),
    }
}

pub(super) fn unsupported_parameter_type_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "native lowering can only lower `i32`, `u8`, `usize`, `bool`, `&str`, `&[T]`, `&+[T]`, scalar borrow parameters, aggregate borrow parameters, and supported aggregate value parameters for function `{function_name}`"
        ),
    )]
}

pub(in crate::ir::lower) fn lower_function_return_type(
    ty: &TypeExpr,
    name: &str,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Result<Type, Vec<Diagnostic>> {
    return_type_from_type_expr_with_resolver(ty, resolved, |source| {
        resolved_sources.get(&source).copied()
    })
    .ok_or_else(|| unsupported_function_return_type_diagnostic(name))
}

pub(super) fn unsupported_function_return_type_diagnostic(name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "native lowering can only lower function `{name}` return type `i32`, `u8`, `usize`, `bool`, `&str`, `&[T]`, `&+[T]`, `void`, `never`, aggregates, or a fallible form of those types"
        ),
    )]
}
