use super::*;
use crate::diagnostics::{Diagnostic, DiagnosticNote};

pub(super) fn missing_program_diagnostic(sources: &SourceMap, span: ByteSpan) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0300",
        "executable root file must define exactly one `program` entry",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "add `program(): i32! { ... }`, `program(): i32 { ... }`, or `program(): void { ... }`"
            .to_string(),
    );
    diagnostic
}

pub(super) fn main_is_not_entry_diagnostic(
    sources: &SourceMap,
    function: &FunctionDecl,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0301",
        "`func main` is an ordinary function; Nocter executable entry uses `program`",
    );
    diagnostic.primary_span = sources.span_to_json(function.name_span).ok().map(Box::new);
    diagnostic.help = Some(
        "replace the entry declaration with `program(): i32! { ... }`, `program(): i32 { ... }`, or `program(): void { ... }`"
            .to_string(),
    );
    diagnostic
}

pub(super) fn duplicate_program_diagnostic(
    sources: &SourceMap,
    first_span: ByteSpan,
    second_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0302",
        "executable root file must not define more than one `program` entry",
    );
    diagnostic.primary_span = sources.span_to_json(second_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first `program` entry is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("keep exactly one top-level `program` declaration".to_string());
    diagnostic
}

pub(super) fn invalid_program_return_type_diagnostic(
    sources: &SourceMap,
    return_type_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0303",
        "`program` return type must be `i32!`, `i32`, or `void` in v0",
    );
    diagnostic.primary_span = sources.span_to_json(return_type_span).ok().map(Box::new);
    diagnostic.help = Some(
        "use `program(): i32!` for a fallible entry point, `program(): i32` for an infallible exit status, or `program(): void` for status 0"
            .to_string(),
    );
    diagnostic
}

pub(super) fn missing_return_value_diagnostic(
    sources: &SourceMap,
    statement: &ReturnStmt,
    context: &ReturnContext,
) -> Diagnostic {
    let expected = context.success_type();
    let mut diagnostic = Diagnostic::error(
        "E0310",
        format!(
            "`return` has no value, but {} returns `{}`",
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(statement.span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!("return a value of type `{}`", expected.display()));
    diagnostic
}

pub(super) fn unexpected_return_value_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0311",
        format!(
            "`return` has a value, but {} returns `void`",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.span()).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("remove the returned value or change the return type".to_string());
    diagnostic
}

pub(super) fn return_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &Expr,
    expected: &Type,
    actual: &Type,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0312",
        format!(
            "`return` value has type `{}`, but {} returns `{}`",
            actual.display(),
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.span()).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!("return a value of type `{}`", expected.display()));
    diagnostic
}

pub(super) fn fail_in_non_fallible_context_diagnostic(
    sources: &SourceMap,
    statement: &FailStmt,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0333",
        format!(
            "`fail` is used in {}, but its return type is not fallible",
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(statement.span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("use `fail` only inside a function returning `T!`".to_string());
    diagnostic
}

pub(super) fn fail_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &FailStmt,
    expected: &Type,
    actual: &Type,
    context: &ReturnContext,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0334",
        format!(
            "`fail` value has type `{}`, but {} fails with `{}`",
            actual.display(),
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.expression.span())
        .ok()
        .map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!(
        "fail with a value of type `{}`",
        expected.display()
    ));
    diagnostic
}

pub(super) fn missing_return_diagnostic(
    sources: &SourceMap,
    block_span: ByteSpan,
    context: &ReturnContext,
) -> Diagnostic {
    let expected = context.success_type();
    let mut diagnostic = Diagnostic::error(
        "E0313",
        format!(
            "{} may reach the end without returning `{}`",
            context.subject(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(block_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some(format!(
        "add a `return` with a value of type `{}`",
        expected.display()
    ));
    diagnostic
}

pub(super) fn argument_count_mismatch_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    signature: &CheckedCallSignature<'_>,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0320",
        format!(
            "{} `{}` expects {expected} argument(s), but call provides {actual}",
            signature.kind.noun(),
            signature.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(call.arguments_span).ok().map(Box::new);
    if let Some(declaration_span) = signature.declaration_span
        && let Ok(span) = sources.span_to_json(declaration_span)
    {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "{} `{}` is declared here",
                signature.kind.noun(),
                signature.name
            ),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "pass exactly the parameters declared by the {}",
        signature.kind.noun()
    ));
    diagnostic
}

pub(super) fn argument_type_mismatch_diagnostic(
    sources: &SourceMap,
    index: usize,
    argument: &Expr,
    parameter: &ParameterSignature,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0321",
        format!(
            "argument {} has type `{}`, but parameter `{}` expects `{}`",
            index + 1,
            actual.display(),
            parameter.name,
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(argument.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(parameter.ty.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("parameter `{}` is declared here", parameter.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!("pass a value of type `{}`", expected.display()));
    diagnostic
}

pub(super) fn binding_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0342",
        format!(
            "`{keyword}` binding `{}` is annotated as `{}`, but the initializer has type `{}`",
            statement.name,
            expected.display(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    if let Some(annotation) = &statement.ty
        && let Ok(span) = sources.span_to_json(annotation.span())
    {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("binding `{}` is annotated here", statement.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "change the initializer or annotate `{}` as `{}`",
        statement.name,
        actual.display()
    ));
    diagnostic
}

pub(super) fn array_literal_element_type_mismatch_diagnostic(
    sources: &SourceMap,
    element: &Expr,
    element_type: &Type,
    first_element: &Expr,
    first_type: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0343",
        format!(
            "array literal element has type `{}`, but earlier elements have type `{}`",
            element_type.display(),
            first_type.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(element.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_element.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "array element type was inferred as `{}` here",
                first_type.display()
            ),
            span: Some(span),
        });
    }
    diagnostic.help = Some("make every array element have the same type".to_string());
    diagnostic
}

pub(super) fn index_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    index: &IndexExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0344",
        format!(
            "index expression target has type `{}`, but indexing requires `[T; N]`, `[T]`, `[+T]`, or `str`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(index.object.span()).ok().map(Box::new);
    diagnostic.help = Some("index an array, view, or string value".to_string());
    diagnostic
}

pub(super) fn index_value_type_mismatch_diagnostic(
    sources: &SourceMap,
    index: &IndexExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0345",
        format!(
            "index expression uses `{}` as the index, but indexes must be integer values",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(index.index_span).ok().map(Box::new);
    diagnostic.help = Some("use an integer value as the index".to_string());
    diagnostic
}

pub(super) fn if_condition_type_mismatch_diagnostic(
    sources: &SourceMap,
    condition: &Expr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0346",
        format!(
            "`if` condition has type `{}`, but conditions must be `bool`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(condition.span()).ok().map(Box::new);
    diagnostic.help = Some("use a `bool` expression as the condition".to_string());
    diagnostic
}

pub(super) fn while_condition_type_mismatch_diagnostic(
    sources: &SourceMap,
    condition: &Expr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0357",
        format!(
            "`while` condition has type `{}`, but conditions must be `bool`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(condition.span()).ok().map(Box::new);
    diagnostic.help = Some("use a `bool` expression as the condition".to_string());
    diagnostic
}

pub(super) fn loop_control_outside_loop_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    keyword: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0359",
        format!("`{keyword}` can only be used inside a loop"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(format!("move `{keyword}` inside a loop body"));
    diagnostic
}

pub(super) fn switch_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &SwitchStmt,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0361",
        format!(
            "`switch` target has type `{}`, but `switch` requires an enum value",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.expression.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("switch on a value whose type is an enum".to_string());
    diagnostic
}

pub(super) fn switch_arm_unknown_enum_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!("`switch` arm refers to unknown enum `{}`", arm.enum_name),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help = Some("use a visible enum type in the arm pattern".to_string());
    diagnostic
}

pub(super) fn switch_arm_non_enum_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!(
            "`switch` arm refers to `{}`, but that type is `{}`",
            arm.enum_name,
            type_symbol_kind_name(symbol.kind)
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help = Some("use an enum type in the arm pattern".to_string());
    diagnostic
}

pub(super) fn switch_arm_enum_mismatch_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    expected: &TypeSymbol,
    actual: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0363",
        format!(
            "`switch` arm uses enum `{}`, but the target enum is `{}`",
            actual.canonical_name, expected.canonical_name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.enum_name_span).ok().map(Box::new);
    diagnostic.help =
        Some("make every arm use the same enum type as the switch target".to_string());
    diagnostic
}

pub(super) fn switch_arm_unknown_variant_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    enum_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0364",
        format!(
            "enum `{}` has no variant `{}`",
            enum_symbol.canonical_name, arm.variant_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(arm.variant_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a variant declared by the enum".to_string());
    diagnostic
}

pub(super) fn switch_arm_payload_mismatch_diagnostic(
    sources: &SourceMap,
    arm: &SwitchArm,
    enum_symbol: &TypeSymbol,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0365",
        format!(
            "`{}.{}` has {} payload value(s), but the switch arm provides {} binding(s)",
            enum_symbol.canonical_name, arm.variant_name, expected, actual
        ),
    );
    diagnostic.primary_span = sources.span_to_json(arm.span).ok().map(Box::new);
    diagnostic.help = Some(
        "match the variant payload shape; v0 supports either no payload or one payload binding"
            .to_string(),
    );
    diagnostic
}

pub(super) fn if_is_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0361",
        format!(
            "`if is` target has type `{}`, but `if is` requires an enum value",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.expression.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use `if value is Enum.variant` with an enum value".to_string());
    diagnostic
}

pub(super) fn if_is_unknown_enum_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!(
            "`if is` pattern refers to unknown enum `{}`",
            statement.enum_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.enum_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a visible enum type in the pattern".to_string());
    diagnostic
}

pub(super) fn if_is_non_enum_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0362",
        format!(
            "`if is` pattern refers to `{}`, but that type is `{}`",
            statement.enum_name,
            type_symbol_kind_name(symbol.kind)
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.enum_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use an enum type in the pattern".to_string());
    diagnostic
}

pub(super) fn if_is_enum_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    expected: &TypeSymbol,
    actual: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0363",
        format!(
            "`if is` pattern uses enum `{}`, but the target enum is `{}`",
            actual.canonical_name, expected.canonical_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.enum_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("make the pattern use the same enum type as the target".to_string());
    diagnostic
}

pub(super) fn if_is_unknown_variant_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    enum_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0364",
        format!(
            "enum `{}` has no variant `{}`",
            enum_symbol.canonical_name, statement.variant_name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.variant_name_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a variant declared by the enum".to_string());
    diagnostic
}

pub(super) fn if_is_payload_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &IfIsStmt,
    enum_symbol: &TypeSymbol,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0365",
        format!(
            "`{}.{}` has {} payload value(s), but the if-is pattern provides {} binding(s)",
            enum_symbol.canonical_name, statement.variant_name, expected, actual
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.pattern_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "match the variant payload shape; v0 supports either no payload or one payload binding"
            .to_string(),
    );
    diagnostic
}

pub(super) fn enum_variant_unknown_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    enum_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0366",
        format!(
            "enum `{}` has no variant `{}`",
            enum_symbol.canonical_name, member.member
        ),
    );
    diagnostic.primary_span = sources.span_to_json(member.member_span).ok().map(Box::new);
    diagnostic.help = Some("use a variant declared by the enum".to_string());
    diagnostic
}

pub(super) fn enum_variant_payload_count_mismatch_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
    expected: usize,
    actual: usize,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0367",
        format!(
            "`{}.{}` expects {} payload value(s), but construction provides {}",
            enum_symbol.canonical_name, variant.name, expected, actual
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(variant.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "variant is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("construct the variant with the payload values declared by the enum".to_string());
    diagnostic
}

pub(super) fn enum_variant_payloadless_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0367",
        format!(
            "`{}.{}` has no payload and must be constructed without `()`",
            enum_symbol.canonical_name, variant.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(call.arguments_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(variant.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "payloadless variant is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "write `{}.{}` instead",
        enum_symbol.canonical_name, variant.name
    ));
    diagnostic
}

pub(super) fn enum_variant_payload_type_mismatch_diagnostic(
    sources: &SourceMap,
    argument: &Expr,
    enum_symbol: &TypeSymbol,
    variant: &EnumVariantSignature,
    index: usize,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0368",
        format!(
            "`{}.{}` payload {} has type `{}`, but the variant expects `{}`",
            enum_symbol.canonical_name,
            variant.name,
            index + 1,
            actual.display(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(argument.span()).ok().map(Box::new);
    if let Some(parameter) = variant.payload.get(index)
        && let Ok(span) = sources.span_to_json(parameter.ty.span())
    {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("payload `{}` is declared here", parameter.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "pass a payload value of type `{}`",
        expected.display()
    ));
    diagnostic
}

pub(super) fn error_member_unknown_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
) -> Diagnostic {
    let mut diagnostic =
        Diagnostic::error("E0369", format!("`error` has no field `{}`", member.member));
    diagnostic.primary_span = sources.span_to_json(member.member_span).ok().map(Box::new);
    diagnostic.help = Some("use `error.code` or `error.message`".to_string());
    diagnostic
}

pub(super) fn struct_field_unknown_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    struct_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0370",
        format!(
            "struct `{}` has no field `{}`",
            struct_symbol.canonical_name, member.member
        ),
    );
    diagnostic.primary_span = sources.span_to_json(member.member_span).ok().map(Box::new);
    diagnostic.help = Some("use a field declared by the struct".to_string());
    diagnostic
}

pub(super) fn method_receiver_unsupported_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    owner: &TypeSymbol,
    method: &MethodSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0377",
        format!(
            "method `{}.{}` uses unsupported receiver type `{}`",
            owner.canonical_name,
            method.name,
            type_expr_display_lossy(&method.receiver.ty)
        ),
    );
    diagnostic.primary_span = sources.span_to_json(member.member_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(method.receiver.ty.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: "receiver type is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("v0 method calls require receiver type `Self`, `&Self`, or `&+Self`".to_string());
    diagnostic
}

pub(super) fn method_readwrite_receiver_requires_var_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    owner: &TypeSymbol,
    method: &MethodSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0378",
        format!(
            "method `{}.{}` requires a mutable `var` receiver",
            owner.canonical_name, method.name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(member.object.span())
        .ok()
        .map(Box::new);
    if let Ok(span) = sources.span_to_json(method.receiver.ty.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: "readwrite receiver is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("bind the receiver with `var` before calling this method".to_string());
    diagnostic
}

pub(super) fn member_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    member: &MemberExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0371",
        format!(
            "field access target has type `{}`, but fields require a struct value",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(member.object.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("access fields on a struct value".to_string());
    diagnostic
}

pub(super) fn struct_literal_target_type_mismatch_diagnostic(
    sources: &SourceMap,
    literal: &StructLiteralExpr,
    actual: &Type,
    resolved: &ResolveOutput,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0372",
        format!(
            "struct literal target has type `{}`, but struct literals require a struct type",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(literal.ty.span()).ok().map(Box::new);
    if let Type::Named(name) = actual
        && let Some(symbol) = resolved.type_symbol_by_canonical_name(name)
    {
        diagnostic.help = Some(format!(
            "`{}` is a {}; use a struct type in the literal",
            symbol.canonical_name,
            type_symbol_kind_name(symbol.kind)
        ));
    } else {
        diagnostic.help = Some("use a struct type before `{ ... }`".to_string());
    }
    diagnostic
}

pub(super) fn struct_literal_unknown_field_diagnostic(
    sources: &SourceMap,
    field: &StructLiteralField,
    struct_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0373",
        format!(
            "struct `{}` has no field `{}`",
            struct_symbol.canonical_name, field.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(field.name_span).ok().map(Box::new);
    diagnostic.help = Some("initialize a field declared by the struct".to_string());
    diagnostic
}

pub(super) fn struct_literal_duplicate_field_diagnostic(
    sources: &SourceMap,
    field: &StructLiteralField,
    first: &StructLiteralField,
    struct_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0374",
        format!(
            "struct `{}` field `{}` is initialized more than once",
            struct_symbol.canonical_name, field.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(field.name_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first initialization is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("initialize each struct field exactly once".to_string());
    diagnostic
}

pub(super) fn struct_literal_missing_field_diagnostic(
    sources: &SourceMap,
    literal: &StructLiteralExpr,
    struct_symbol: &TypeSymbol,
    field: &StructFieldSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0375",
        format!(
            "struct `{}` literal does not initialize field `{}`",
            struct_symbol.canonical_name, field.name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(literal.fields_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(field.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("field `{}` is declared here", field.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!("add `{}` to the struct literal", field.name));
    diagnostic
}

pub(super) fn struct_literal_field_type_mismatch_diagnostic(
    sources: &SourceMap,
    field: &StructLiteralField,
    expected_field: &StructFieldSignature,
    expected: &Type,
    actual: &Type,
    struct_symbol: &TypeSymbol,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0376",
        format!(
            "struct `{}` field `{}` is initialized with `{}`, but the field expects `{}`",
            struct_symbol.canonical_name,
            field.name,
            actual.display(),
            expected.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(field.value.span()).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(expected_field.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("field `{}` is declared here", expected_field.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "initialize `{}` with a value of type `{}`",
        field.name,
        expected.display()
    ));
    diagnostic
}

pub(super) fn struct_literal_inaccessible_field_diagnostic(
    sources: &SourceMap,
    field_span: ByteSpan,
    struct_symbol: &TypeSymbol,
    field: &StructFieldSignature,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0377",
        format!(
            "field `{}` of struct `{}` is not visible here",
            field.name, struct_symbol.canonical_name
        ),
    );
    diagnostic.primary_span = sources.span_to_json(field_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(field.name_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("field `{}` is declared here", field.name),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("construct this value through a visible API from its defining module".to_string());
    diagnostic
}

pub(super) fn struct_literal_inaccessible_missing_field_diagnostic(
    sources: &SourceMap,
    literal: &StructLiteralExpr,
    struct_symbol: &TypeSymbol,
    field: &StructFieldSignature,
) -> Diagnostic {
    let mut diagnostic = struct_literal_inaccessible_field_diagnostic(
        sources,
        literal.ty.span(),
        struct_symbol,
        field,
    );
    diagnostic.message = format!(
        "struct `{}` literal cannot initialize hidden field `{}`",
        struct_symbol.canonical_name, field.name
    );
    diagnostic
}

pub(super) fn type_symbol_kind_name(kind: TypeSymbolKind) -> &'static str {
    match kind {
        TypeSymbolKind::Alias => "type alias",
        TypeSymbolKind::Struct => "struct",
        TypeSymbolKind::Enum => "enum",
        TypeSymbolKind::Trait => "trait",
    }
}

pub(super) fn for_range_bound_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &ForRangeStmt,
    start_type: &Type,
    end_type: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0360",
        format!(
            "`for` range bounds have types `{}` and `{}`, but range `for` requires matching integer bounds",
            start_type.display(),
            end_type.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.range_span)
        .ok()
        .map(Box::new);
    if let Ok(span) = sources.span_to_json(statement.start.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("range start has type `{}`", start_type.display()),
            span: Some(span),
        });
    }
    if let Ok(span) = sources.span_to_json(statement.end.span()) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("range end has type `{}`", end_type.display()),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("use integer bounds with the same type, or an integer literal that fits the other bound type".to_string());
    diagnostic
}

pub(super) fn equality_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0347",
        format!(
            "operator `{}` compares `{}` with `{}`, but equality operands must use the same supported equality type",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "compare `bool`, integer, `str`, or supported payloadless enum values of the same type"
            .to_string(),
    );
    diagnostic
}

pub(super) fn arithmetic_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0352",
        format!(
            "operator `{}` combines `{}` with `{}`, but integer arithmetic requires matching integer operands",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use integer operands with the same type".to_string());
    diagnostic
}

pub(super) fn shift_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0353",
        format!(
            "operator `{}` shifts `{}` by `{}`, but shift operands must be integer values",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("shift an integer value by an integer count".to_string());
    diagnostic
}

pub(super) fn negative_shift_count_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0354",
        format!(
            "operator `{}` uses a negative shift count",
            expression.operator.spelling()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.right.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a non-negative shift count".to_string());
    diagnostic
}

pub(super) fn type_conversion_not_lossless_diagnostic(
    sources: &SourceMap,
    expression: &TypeConversionExpr,
    source: &Type,
    target: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0355",
        format!(
            "`as` conversion from `{}` to `{}` is not a lossless integer conversion",
            source.display(),
            target.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(expression.as_span).ok().map(Box::new);
    diagnostic.help = Some(
        "use `as` only when every source value can be represented by the target type".to_string(),
    );
    diagnostic
}

pub(super) fn ordered_comparison_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0348",
        format!(
            "operator `{}` compares `{}` with `{}`, but ordered comparison requires matching integer operands",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("compare integer values with the same type".to_string());
    diagnostic
}

pub(super) fn logical_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &BinaryExpr,
    left: &Type,
    right: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0349",
        format!(
            "operator `{}` combines `{}` with `{}`, but logical operators require `bool` operands",
            expression.operator.spelling(),
            left.display(),
            right.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use `bool` expressions on both sides".to_string());
    diagnostic
}

pub(super) fn logical_not_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &UnaryExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0350",
        format!(
            "operator `{}` uses `{}`, but logical not requires a `bool` operand",
            expression.operator.spelling(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a `bool` expression after `!`".to_string());
    diagnostic
}

pub(super) fn numeric_negate_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    expression: &UnaryExpr,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0351",
        format!(
            "operator `{}` uses `{}`, but numeric negation requires a signed integer operand",
            expression.operator.spelling(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some("use a signed integer value after `-`".to_string());
    diagnostic
}

pub(super) fn try_on_non_fallible_diagnostic(
    sources: &SourceMap,
    try_span: ByteSpan,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0330",
        format!(
            "fallible handling requires a fallible expression, but this expression has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(try_span).ok().map(Box::new);
    diagnostic.help = Some(
        "remove postfix `?` or `catch`, or call a function whose return type is `T!`".to_string(),
    );
    diagnostic
}

pub(super) fn try_in_non_fallible_context_diagnostic(
    sources: &SourceMap,
    try_span: ByteSpan,
    context: &ReturnContext,
    attempted_error: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0331",
        format!(
            "postfix `?` would fail with `{}`, but {} is not fallible",
            attempted_error.display(),
            context.subject()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(try_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help =
        Some("add `catch error { ... }` or make the current callable return `T!`".to_string());
    diagnostic
}

pub(super) fn try_error_type_mismatch_diagnostic(
    sources: &SourceMap,
    try_span: ByteSpan,
    context: &ReturnContext,
    current_error: &Type,
    attempted_error: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0332",
        format!(
            "postfix `?` would fail with `{}`, but {} fails with `{}`",
            attempted_error.display(),
            context.subject(),
            current_error.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(try_span).ok().map(Box::new);
    add_declared_return_note(sources, &mut diagnostic, context);
    diagnostic.help = Some("handle the failure with `catch`".to_string());
    diagnostic
}

pub(super) fn optional_if_let_non_optional_diagnostic(
    sources: &SourceMap,
    statement: &IfLetStmt,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0356",
        format!(
            "`if {keyword}` requires an optional initializer, but the initializer has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "use an initializer whose type is `T?`, or use a regular `if` condition instead of `if {keyword}`"
    ));
    diagnostic
}

pub(super) fn optional_while_let_non_optional_diagnostic(
    sources: &SourceMap,
    statement: &WhileLetStmt,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0358",
        format!(
            "`while {keyword}` requires an optional initializer, but the initializer has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "use an initializer whose type is `T?`, or use a regular `while` condition instead of `while {keyword}`"
    ));
    diagnostic
}

pub(super) fn optional_let_else_non_optional_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0340",
        format!(
            "`{keyword} ... else` requires an optional initializer, but the initializer has type `{}`",
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "remove `else`, or use an initializer whose type is `T?` for `{keyword} ... else`"
    ));
    diagnostic
}

pub(super) fn optional_let_else_fallthrough_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    else_block: &Block,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0341",
        format!("`{keyword} ... else` requires an `else` block that cannot fall through"),
    );
    diagnostic.primary_span = sources.span_to_json(else_block.span).ok().map(Box::new);
    diagnostic.help = Some(
        "end the `else` block with `return` or `fail` in parser/check v0; later phases will add `break`, `continue`, and `never` support"
            .to_string(),
    );
    diagnostic
}

pub(super) fn binding_keyword(kind: BindingKind) -> &'static str {
    match kind {
        BindingKind::Let => "let",
        BindingKind::Var => "var",
    }
}

pub(super) fn add_declared_return_note(
    sources: &SourceMap,
    diagnostic: &mut Diagnostic,
    context: &ReturnContext,
) {
    if let Ok(span) = sources.span_to_json(context.return_type_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: format!(
                "{} declares return type `{}`",
                context.subject(),
                context.declared_type.display()
            ),
            span: Some(span),
        });
    }
}
