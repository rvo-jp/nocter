use super::*;

pub(super) fn validate_supported_fallible_success_payload_abi(
    success_type: &Type,
) -> Result<(), Vec<Diagnostic>> {
    match success_type.success_return_passing() {
        Some(ReturnPassing::Void | ReturnPassing::IndirectPointer) => Ok(()),
        Some(ReturnPassing::Direct { words }) => {
            if words <= FALLIBLE_SUCCESS_PAYLOAD_REGISTER_COUNT {
                return Ok(());
            }
            Err(vec![Diagnostic::error(
                "E9002",
                format!(
                    "fallible success payload uses {words} direct ABI words, but codegen supports at most {FALLIBLE_SUCCESS_PAYLOAD_REGISTER_COUNT}"
                ),
            )])
        }
        Some(ReturnPassing::Never) | None => Err(vec![Diagnostic::error(
            "E9002",
            "invalid fallible success payload ABI for codegen",
        )]),
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ExpectedCallReturnShape {
    I32,
    U8,
    Usize,
    Borrow,
    Bool,
    Str,
    Slice,
    Void,
    IndirectAggregate,
    DirectAggregate { layout: crate::abi::ValueLayout },
}

impl ExpectedCallReturnShape {
    fn passing(self) -> Option<ReturnPassing> {
        match self {
            Self::I32 | Self::U8 | Self::Usize | Self::Borrow | Self::Bool => {
                Some(ReturnPassing::Direct { words: 1 })
            }
            Self::Str | Self::Slice => Some(ReturnPassing::Direct { words: 2 }),
            Self::Void => Some(ReturnPassing::Void),
            Self::IndirectAggregate => Some(ReturnPassing::IndirectPointer),
            Self::DirectAggregate { layout } => direct_aggregate_layout_passing(layout),
        }
    }

    fn matches_success_type(self, ty: &Type) -> bool {
        match (self, ty) {
            (Self::I32, Type::I32)
            | (Self::U8, Type::U8)
            | (Self::Usize, Type::Usize)
            | (Self::Borrow, Type::Borrow { .. })
            | (Self::Bool, Type::Bool)
            | (Self::Str, Type::Str)
            | (Self::Slice, Type::Slice { .. })
            | (Self::Void, Type::Void)
            | (Self::IndirectAggregate, Type::Aggregate { .. }) => true,
            (Self::DirectAggregate { layout }, Type::DirectAggregate { layout: actual, .. }) => {
                layout == *actual
            }
            _ => false,
        }
    }

    fn description(self) -> String {
        match self {
            Self::I32 => "i32".to_string(),
            Self::U8 => "u8".to_string(),
            Self::Usize => "usize".to_string(),
            Self::Borrow => "borrow".to_string(),
            Self::Bool => "bool".to_string(),
            Self::Str => "&str".to_string(),
            Self::Slice => "slice".to_string(),
            Self::Void => "void".to_string(),
            Self::IndirectAggregate => "indirect aggregate".to_string(),
            Self::DirectAggregate { layout } => format!(
                "direct aggregate {} ({})",
                layout_description(layout),
                return_passing_description(self.passing())
            ),
        }
    }
}

pub(super) fn validate_module_call_return_shapes(module: &IrModule) -> Result<(), Vec<Diagnostic>> {
    let return_types = module
        .functions
        .iter()
        .map(|function| {
            (
                FunctionSymbol::from_function(function),
                &function.return_type,
            )
        })
        .collect::<HashMap<_, _>>();
    let mut diagnostics = Vec::new();

    for function in &module.functions {
        validate_function_return_type_shape(function, &mut diagnostics);
        validate_instruction_list_call_return_shapes(
            &function.instructions,
            &function.return_type,
            &return_types,
            &mut diagnostics,
        );
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(super) fn validate_function_return_type_shape(
    function: &Function,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_return_type_shape(
        &function.return_type,
        &format!("function `{}` return type", function.name),
        diagnostics,
    );
}

pub(super) fn validate_return_type_shape(
    ty: &Type,
    subject: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match ty {
        Type::DirectAggregate { layout, words } => {
            validate_direct_aggregate_type_shape(*layout, *words, subject, diagnostics);
        }
        Type::Fallible(success) => {
            let subject = format!("{subject} fallible success type");
            validate_return_type_shape(success, &subject, diagnostics);
        }
        Type::ComposedOutcome { payload, .. } => {
            let subject = format!("{subject} composed outcome payload type");
            validate_return_type_shape(payload, &subject, diagnostics);
        }
        _ => {}
    }
}

pub(super) fn validate_direct_aggregate_type_shape(
    layout: crate::abi::ValueLayout,
    words: usize,
    subject: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(expected_words) = direct_aggregate_layout_word_count(layout) else {
        diagnostics.push(Diagnostic::error(
            "E9002",
            format!("codegen {subject} layout exceeds host word count range"),
        ));
        return;
    };
    if expected_words > DIRECT_AGGREGATE_REGISTER_WORD_COUNT {
        diagnostics.push(Diagnostic::error(
            "E9002",
            format!(
                "codegen {subject} requires {expected_words} direct ABI words, but direct aggregate codegen supports at most {DIRECT_AGGREGATE_REGISTER_WORD_COUNT}"
            ),
        ));
        return;
    }
    if words == expected_words {
        return;
    }
    diagnostics.push(Diagnostic::error(
        "E9002",
        format!(
            "codegen {subject} uses {words} ABI words, but layout {} requires {expected_words}",
            layout_description(layout),
        ),
    ));
}

pub(super) fn validate_instruction_list_call_return_shapes(
    instructions: &[Instruction],
    current_return_type: &Type,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for instruction in instructions {
        if let Some(arguments) = instruction_call_arguments(instruction) {
            validate_call_argument_shapes(arguments, diagnostics);
        }

        match instruction {
            Instruction::CallI32 { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::I32,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleI32 {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::I32,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallU8 { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::U8,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleU8 {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::U8,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallUsize { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Usize,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleUsize {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Usize,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallBorrow { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Borrow,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleBorrow {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Borrow,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::ReadSlice { failure_mode, .. } => {
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::OpenRead { failure_mode, .. } => {
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallBool { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Bool,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleBool {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Bool,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallStr { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Str,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleStr {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Str,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallSlice { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Slice,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleSlice {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Slice,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallAggregate { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::IndirectAggregate,
                return_types,
                diagnostics,
            ),
            Instruction::CallDirectAggregate { target, layout, .. } => {
                validate_normal_call_return_shape(
                    target,
                    ExpectedCallReturnShape::DirectAggregate { layout: *layout },
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallFallibleDirectAggregate {
                target,
                layout,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::DirectAggregate { layout: *layout },
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallFallibleAggregate {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::IndirectAggregate,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallVoid { target, .. } => validate_normal_call_return_shape(
                target,
                ExpectedCallReturnShape::Void,
                return_types,
                diagnostics,
            ),
            Instruction::CallFallibleVoid {
                target,
                failure_mode,
                ..
            } => {
                validate_fallible_call_return_shape(
                    target,
                    ExpectedCallReturnShape::Void,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    failure_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CallComposedOutcome {
                destination,
                target,
                outer,
                inner,
                outer_mode,
                inner_mode,
                ..
            } => {
                validate_composed_outcome_call_return_shape(
                    target,
                    *destination,
                    *outer,
                    *inner,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    outer_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
                validate_failure_mode_call_return_shapes(
                    inner_mode,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::TailCall { target, .. } => {
                validate_tail_call_return_shape(
                    target,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                validate_instruction_list_call_return_shapes(
                    then_instructions,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
                validate_instruction_list_call_return_shapes(
                    else_instructions,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::While {
                condition_instructions,
                body_instructions,
                ..
            } => {
                validate_instruction_list_call_return_shapes(
                    condition_instructions,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
                validate_instruction_list_call_return_shapes(
                    body_instructions,
                    current_return_type,
                    return_types,
                    diagnostics,
                );
            }
            Instruction::CheckFailure { failure_mode } => validate_failure_mode_call_return_shapes(
                failure_mode,
                current_return_type,
                return_types,
                diagnostics,
            ),
            _ => {}
        }
    }
}

pub(super) fn instruction_call_arguments(instruction: &Instruction) -> Option<&[ScalarArgument]> {
    match instruction {
        Instruction::CallI32 { arguments, .. }
        | Instruction::CallFallibleI32 { arguments, .. }
        | Instruction::CallU8 { arguments, .. }
        | Instruction::CallFallibleU8 { arguments, .. }
        | Instruction::CallUsize { arguments, .. }
        | Instruction::CallFallibleUsize { arguments, .. }
        | Instruction::CallBool { arguments, .. }
        | Instruction::CallFallibleBool { arguments, .. }
        | Instruction::CallStr { arguments, .. }
        | Instruction::CallFallibleStr { arguments, .. }
        | Instruction::CallSlice { arguments, .. }
        | Instruction::CallFallibleSlice { arguments, .. }
        | Instruction::CallAggregate { arguments, .. }
        | Instruction::CallDirectAggregate { arguments, .. }
        | Instruction::CallFallibleDirectAggregate { arguments, .. }
        | Instruction::CallFallibleAggregate { arguments, .. }
        | Instruction::CallVoid { arguments, .. }
        | Instruction::CallFallibleVoid { arguments, .. }
        | Instruction::CallComposedOutcome { arguments, .. }
        | Instruction::TailCall { arguments, .. } => Some(arguments),
        _ => None,
    }
}

pub(super) fn validate_call_argument_shapes(
    arguments: &[ScalarArgument],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for argument in arguments {
        if let ScalarArgument::AggregateDirect(argument) = argument {
            validate_direct_aggregate_argument_shape(argument, diagnostics);
        }
    }
}

pub(super) fn validate_direct_aggregate_argument_shape(
    argument: &DirectAggregateArgument,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(expected_words) = direct_aggregate_layout_word_count(argument.layout) else {
        diagnostics.push(Diagnostic::error(
            "E9002",
            "codegen direct aggregate argument layout exceeds host word count range",
        ));
        return;
    };
    if argument.words == expected_words {
        return;
    }
    diagnostics.push(Diagnostic::error(
        "E9002",
        format!(
            "codegen direct aggregate argument uses {} ABI words, but layout {} requires {expected_words}",
            argument.words,
            layout_description(argument.layout),
        ),
    ));
}

pub(super) fn validate_failure_mode_call_return_shapes(
    failure_mode: &FallibleFailureMode,
    current_return_type: &Type,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => {}
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Recover { instructions }
        | FallibleFailureMode::Catch { instructions, .. } => {
            validate_instruction_list_call_return_shapes(
                instructions,
                current_return_type,
                return_types,
                diagnostics,
            );
        }
    }
}

pub(super) fn validate_normal_call_return_shape(
    target: &CallTarget,
    expected: ExpectedCallReturnShape,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let function = FunctionSymbol::from_call_target(target);
    let Some(return_type) = return_types.get(&function) else {
        diagnostics.push(unresolved_call_target_diagnostic(&function));
        return;
    };
    if matches!(
        return_type,
        Type::Fallible(_) | Type::ComposedOutcome { .. }
    ) {
        let outcome = match return_type {
            Type::Fallible(_) => "fallible",
            Type::ComposedOutcome { .. } => "composed outcome",
            _ => unreachable!(),
        };
        diagnostics.push(Diagnostic::error(
            "E9002",
            format!(
                "codegen normal call to function `{}` targets a {outcome} return",
                function.description(),
            ),
        ));
        return;
    }
    validate_success_return_shape(&function, return_type, expected, "normal call", diagnostics);
}

pub(super) fn validate_fallible_call_return_shape(
    target: &CallTarget,
    expected: ExpectedCallReturnShape,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let function = FunctionSymbol::from_call_target(target);
    let Some(return_type) = return_types.get(&function) else {
        diagnostics.push(unresolved_call_target_diagnostic(&function));
        return;
    };
    let Type::Fallible(success_type) = return_type else {
        diagnostics.push(Diagnostic::error(
            "E9002",
            format!(
                "codegen fallible call to function `{}` targets a non-fallible return",
                function.description()
            ),
        ));
        return;
    };
    validate_success_return_shape(
        &function,
        success_type,
        expected,
        "fallible call success",
        diagnostics,
    );
}

pub(super) fn validate_composed_outcome_call_return_shape(
    target: &CallTarget,
    destination: ComposedOutcomeDestination,
    expected_outer: crate::outcomes::OutcomeLayer,
    expected_inner: crate::outcomes::OutcomeLayer,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let function = FunctionSymbol::from_call_target(target);
    let Some(return_type) = return_types.get(&function) else {
        diagnostics.push(unresolved_call_target_diagnostic(&function));
        return;
    };
    let Type::ComposedOutcome {
        outer,
        inner,
        payload,
    } = return_type
    else {
        diagnostics.push(Diagnostic::error(
            "E9002",
            format!(
                "codegen composed outcome call to function `{}` targets a non-composed return",
                function.description()
            ),
        ));
        return;
    };
    if (*outer, *inner) != (expected_outer, expected_inner) {
        diagnostics.push(Diagnostic::error(
            "E9002",
            format!(
                "codegen composed outcome call layer mismatch for function `{}`",
                function.description()
            ),
        ));
        return;
    }
    let expected = match destination {
        ComposedOutcomeDestination::I32(_) => ExpectedCallReturnShape::I32,
        ComposedOutcomeDestination::U8(_) => ExpectedCallReturnShape::U8,
        ComposedOutcomeDestination::Usize(_) => ExpectedCallReturnShape::Usize,
        ComposedOutcomeDestination::Borrow(_) => ExpectedCallReturnShape::Borrow,
        ComposedOutcomeDestination::Bool(_) => ExpectedCallReturnShape::Bool,
        ComposedOutcomeDestination::Str(_) => ExpectedCallReturnShape::Str,
        ComposedOutcomeDestination::Slice(_) => ExpectedCallReturnShape::Slice,
    };
    validate_success_return_shape(
        &function,
        payload,
        expected,
        "composed outcome call payload",
        diagnostics,
    );
}

pub(super) fn validate_tail_call_return_shape(
    target: &CallTarget,
    current_return_type: &Type,
    return_types: &HashMap<FunctionSymbol, &Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let function = FunctionSymbol::from_call_target(target);
    let Some(return_type) = return_types.get(&function) else {
        diagnostics.push(unresolved_call_target_diagnostic(&function));
        return;
    };
    if return_type.success_return_passing() == Some(ReturnPassing::Never) {
        return;
    }
    if *return_type == current_return_type {
        return;
    }
    diagnostics.push(Diagnostic::error(
        "E9002",
        format!(
            "codegen tail call return mismatch for function `{}`: expected {}, got {}",
            function.description(),
            type_return_description(current_return_type),
            type_return_description(return_type),
        ),
    ));
}

pub(super) fn validate_success_return_shape(
    function: &FunctionSymbol,
    success_type: &Type,
    expected: ExpectedCallReturnShape,
    context: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expected_passing = expected.passing();
    let actual_passing = success_type.success_return_passing();
    if expected.matches_success_type(success_type) && expected_passing == actual_passing {
        return;
    }
    diagnostics.push(Diagnostic::error(
        "E9002",
        format!(
            "codegen {context} return mismatch for function `{}`: expected {} ({}), got {}",
            function.description(),
            expected.description(),
            return_passing_description(expected_passing),
            type_return_description(success_type),
        ),
    ));
}

pub(super) fn direct_aggregate_layout_passing(
    layout: crate::abi::ValueLayout,
) -> Option<ReturnPassing> {
    direct_aggregate_layout_word_count(layout).map(|words| ReturnPassing::Direct { words })
}

pub(super) fn direct_aggregate_layout_word_count(layout: crate::abi::ValueLayout) -> Option<usize> {
    usize::try_from(layout.size.div_ceil(crate::abi::ABI_WORD_SIZE)).ok()
}

pub(super) fn registers_overlap(destinations: &[XReg], sources: &[Option<XReg>; 2]) -> bool {
    destinations
        .iter()
        .any(|destination| sources.iter().any(|source| source == &Some(*destination)))
}

pub(super) fn failure_payload_temporary_pair(
    protected_sources: &[Option<XReg>; 2],
    protected_destinations: &[XReg; 2],
) -> Result<(XReg, XReg), Vec<Diagnostic>> {
    let candidates = [
        XReg::X5,
        XReg::X6,
        XReg::X7,
        XReg::X9,
        XReg::X10,
        XReg::X11,
        XReg::X12,
        XReg::X13,
        XReg::X14,
        XReg::X15,
    ];
    let selected = candidates
        .into_iter()
        .filter(|register| {
            !protected_destinations.contains(register)
                && !protected_sources
                    .iter()
                    .any(|source| source == &Some(*register))
        })
        .take(2)
        .collect::<Vec<_>>();

    let [ptr, len] = selected.as_slice() else {
        return Err(vec![Diagnostic::error(
            "E9005",
            "codegen cannot allocate temporary registers for fallible failure payload",
        )]);
    };
    Ok((*ptr, *len))
}
