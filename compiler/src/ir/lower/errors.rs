use super::context::LoweringContext;
use crate::ast::{CallExpr, Expr, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::StrValue;
use crate::literals::decode_string_literal_bytes;
use crate::resolve::{FunctionSignature, ResolveOutput, SymbolKind};
use crate::source::SourceId;

pub(super) struct ErrorPayload {
    code: StrValue,
    message: StrValue,
}

impl ErrorPayload {
    pub(super) fn into_str_values(self) -> (StrValue, StrValue) {
        (self.code, self.message)
    }
}

pub(super) fn lower_error_payload(
    expression: &Expr,
    resolved: &ResolveOutput,
    root_source: SourceId,
    context: Option<&LoweringContext>,
) -> Result<Option<ErrorPayload>, Vec<Diagnostic>> {
    let Expr::Call(call) = expression else {
        return Ok(None);
    };

    if !is_error_constructor_call(call, resolved, root_source) {
        return Ok(None);
    }

    if call.arguments.len() != 2 {
        return Err(unsupported_fail_payload_diagnostic());
    };

    let code = lower_error_string_value(&call.arguments[0], "code", context)?;
    let message = lower_error_string_value(&call.arguments[1], "message", context)?;

    Ok(Some(ErrorPayload { code, message }))
}

fn is_error_constructor_call(
    call: &CallExpr,
    resolved: &ResolveOutput,
    root_source: SourceId,
) -> bool {
    if let Some(symbol) = resolved.symbol_for_call(call)
        && symbol.declaration_span.source != root_source
        && let SymbolKind::Function(signature) | SymbolKind::Primitive(signature) = &symbol.kind
    {
        return signature_is_static_error_constructor(signature, resolved);
    }

    if let Some((_owner, function)) = resolved.associated_function_for_call(call)
        && function.name_span.source != root_source
    {
        return signature_is_static_error_constructor(&function.signature, resolved);
    }

    false
}

fn signature_is_static_error_constructor(
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> bool {
    signature.parameters.len() == 2 && type_expr_resolves_to_error(&signature.return_type, resolved)
}

fn type_expr_resolves_to_error(ty: &TypeExpr, resolved: &ResolveOutput) -> bool {
    let TypeExpr::Reference(reference) = ty else {
        return false;
    };

    if reference.name == "error" {
        return true;
    }

    resolved
        .type_symbol_by_name(&reference.name)
        .and_then(|symbol| symbol.alias_target.as_ref())
        .is_some_and(|target| type_expr_resolves_to_error(target, resolved))
}

fn lower_error_string_value(
    expression: &Expr,
    field: &str,
    context: Option<&LoweringContext>,
) -> Result<StrValue, Vec<Diagnostic>> {
    match expression {
        Expr::StringLiteral(literal) => decode_string_literal_bytes(&literal.value)
            .map(StrValue::StaticBytes)
            .map_err(|message| {
                vec![Diagnostic::error(
                    "E8005",
                    format!("IR v0 cannot decode failure {field} literal: {message}"),
                )]
            }),
        Expr::Identifier(identifier) => context
            .and_then(|context| context.str_location(&identifier.name))
            .map(StrValue::Location)
            .ok_or_else(unsupported_fail_payload_diagnostic),
        Expr::Member(member) => {
            let Some(context) = context else {
                return Err(unsupported_fail_payload_diagnostic());
            };
            let Expr::Identifier(identifier) = member.object.as_ref() else {
                return Err(unsupported_fail_payload_diagnostic());
            };

            let location = match member.member.as_str() {
                "code" => context.error_code_location(&identifier.name),
                "message" => context.error_message_location(&identifier.name),
                _ => None,
            };

            location
                .map(StrValue::Location)
                .ok_or_else(unsupported_fail_payload_diagnostic)
        }
        Expr::Group(group) => lower_error_string_value(&group.expression, field, context),
        _ => Err(unsupported_fail_payload_diagnostic()),
    }
}

fn unsupported_fail_payload_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8004",
        "IR v0 can only lower fallible failure returns through a loaded error constructor call with string code and message",
    )]
}
