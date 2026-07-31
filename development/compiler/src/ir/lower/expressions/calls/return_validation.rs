use super::*;

pub(super) fn validate_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::I32 {
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `i32`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_usize_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::Usize {
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `usize`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_u8_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::U8 {
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `u8`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_bool_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::Bool {
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `bool`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_str_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::Str {
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning `&str`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_slice_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if matches!(callee_return_type, Type::Slice { .. }) {
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower normal calls returning a slice, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_void_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower expression statements calling functions with known `void` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if callee_return_type == &Type::Void {
        return validate_call_success_return_passing(
            target,
            callee_name,
            callee_return_type,
            context,
        );
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower expression statements calling functions returning `void`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_fallible_void_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated call statements with known `void!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::Void
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated call statements returning `void!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_fallible_i32_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known `i32!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::I32
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning `i32!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_fallible_usize_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known `usize!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::Usize
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning `usize!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_fallible_u8_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known `u8!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::U8
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning `u8!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_fallible_bool_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known `bool!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::Bool
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning `bool!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_fallible_str_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known `&str!` return type, got function `{callee_name}`"
            ),
        )]);
    };

    if let Type::Fallible(success) = callee_return_type
        && success.as_ref() == &Type::Str
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning `&str!`, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_fallible_slice_normal_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Err(vec![Diagnostic::error(
            "E8006",
            format!(
                "IR v0 can only lower propagated calls with known slice fallible return type, got function `{callee_name}`"
            ),
        )]);
    };

    if let Type::Fallible(success) = callee_return_type
        && matches!(success.as_ref(), Type::Slice { .. })
    {
        return validate_call_success_return_passing(target, callee_name, success, context);
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 can only lower propagated calls returning a slice fallible type, got function `{callee_name}` returning `{}`",
            describe_type(callee_return_type),
        ),
    )])
}

pub(super) fn validate_tail_call_return_type(
    target: &CallTarget,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(callee_return_type) = context.call_return_type(target) else {
        return Ok(());
    };

    if callee_return_type == &Type::Never || callee_return_type == context.return_type() {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 cannot lower tail call from function `{}` returning `{}` to function `{callee_name}` returning `{}`",
            context.function_name(),
            describe_type(context.return_type()),
            describe_type(callee_return_type),
        ),
    )])
}

fn validate_call_success_return_passing(
    target: &CallTarget,
    callee_name: &str,
    expected_success_type: &Type,
    context: &LoweringContext,
) -> Result<(), Vec<Diagnostic>> {
    let Some(actual) = context.call_success_return_passing(target) else {
        return Ok(());
    };
    let Some(expected) = expected_success_type.success_return_passing() else {
        return Ok(());
    };
    if actual == expected {
        return Ok(());
    }

    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "IR v0 call return ABI mismatch for function `{callee_name}`: expected callee success return to use `{}`, got `{}`",
            expected.description(),
            actual.description(),
        ),
    )])
}

pub(super) fn describe_type(ty: &Type) -> &'static str {
    match ty {
        Type::I32 => "i32",
        Type::U8 => "u8",
        Type::Usize => "usize",
        Type::Bool => "bool",
        Type::Str => "&str",
        Type::Slice {
            is_readwrite: false,
        } => "&[T]",
        Type::Slice { is_readwrite: true } => "&+[T]",
        Type::Aggregate { .. } => "aggregate",
        Type::DirectAggregate { .. } => "aggregate",
        Type::Error => "error",
        Type::Borrow {
            is_readwrite: false,
            inner,
        } => match inner.as_ref() {
            Type::I32 => "&i32",
            Type::U8 => "&u8",
            Type::Usize => "&usize",
            Type::Bool => "&bool",
            Type::Aggregate { .. } => "&aggregate",
            Type::DirectAggregate { .. } => "&aggregate",
            _ => "borrow",
        },
        Type::Borrow {
            is_readwrite: true,
            inner,
        } => match inner.as_ref() {
            Type::I32 => "&+i32",
            Type::U8 => "&+u8",
            Type::Usize => "&+usize",
            Type::Bool => "&+bool",
            Type::Aggregate { .. } => "&+aggregate",
            Type::DirectAggregate { .. } => "&+aggregate",
            _ => "borrow",
        },
        Type::Void => "void",
        Type::Never => "never",
        Type::Fallible(success) => match success.as_ref() {
            Type::I32 => "i32!",
            Type::U8 => "u8!",
            Type::Usize => "usize!",
            Type::Bool => "bool!",
            Type::Str => "&str!",
            Type::Slice {
                is_readwrite: false,
            } => "&[T]!",
            Type::Slice { is_readwrite: true } => "&+[T]!",
            Type::Aggregate { .. } => "aggregate!",
            Type::DirectAggregate { .. } => "aggregate!",
            Type::Borrow {
                is_readwrite: false,
                inner,
            } => match inner.as_ref() {
                Type::I32 => "&i32!",
                Type::U8 => "&u8!",
                Type::Usize => "&usize!",
                Type::Bool => "&bool!",
                Type::Aggregate { .. } => "&aggregate!",
                Type::DirectAggregate { .. } => "&aggregate!",
                _ => "borrow!",
            },
            Type::Borrow {
                is_readwrite: true,
                inner,
            } => match inner.as_ref() {
                Type::I32 => "&+i32!",
                Type::U8 => "&+u8!",
                Type::Usize => "&+usize!",
                Type::Bool => "&+bool!",
                Type::Aggregate { .. } => "&+aggregate!",
                Type::DirectAggregate { .. } => "&+aggregate!",
                _ => "borrow!",
            },
            Type::Void => "void!",
            Type::Never => "never!",
            Type::Error => "error!",
            Type::Fallible(_) => "fallible",
        },
    }
}
