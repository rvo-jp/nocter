use crate::ast::{CallExpr, Expr, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::literals::decode_string_literal_bytes;
use crate::resolve::{FunctionSignature, ResolveOutput, SymbolKind};
use crate::source::SourceId;

pub(super) struct StaticErrorPayload {
    code: Vec<u8>,
    message: Vec<u8>,
}

impl StaticErrorPayload {
    pub(super) fn report_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.code);
        bytes.extend_from_slice(b": ");
        bytes.extend_from_slice(&self.message);
        with_trailing_newline(bytes)
    }
}

pub(super) fn lower_static_error_payload(
    expression: &Expr,
    resolved: &ResolveOutput,
    root_source: SourceId,
) -> Result<Option<StaticErrorPayload>, Vec<Diagnostic>> {
    let Expr::Call(call) = expression else {
        return Ok(None);
    };

    if !is_static_error_constructor_call(call, resolved, root_source) {
        return Ok(None);
    }

    if call.arguments.len() != 2 {
        return Err(unsupported_fail_payload_diagnostic());
    };

    let code = decode_static_error_string(&call.arguments[0], "code")?;
    let message = decode_static_error_string(&call.arguments[1], "message")?;

    Ok(Some(StaticErrorPayload { code, message }))
}

pub(super) fn with_trailing_newline(mut bytes: Vec<u8>) -> Vec<u8> {
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes
}

fn is_static_error_constructor_call(
    call: &CallExpr,
    resolved: &ResolveOutput,
    root_source: SourceId,
) -> bool {
    if let Some(symbol) = resolved.symbol_for_call(call)
        && symbol.declaration_span.source != root_source
        && let SymbolKind::Function(signature) = &symbol.kind
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

fn decode_static_error_string(expression: &Expr, field: &str) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let Expr::StringLiteral(literal) = expression else {
        return Err(unsupported_fail_payload_diagnostic());
    };

    decode_string_literal_bytes(&literal.value).map_err(|message| {
        vec![Diagnostic::error(
            "E8005",
            format!("IR v0 cannot decode failure {field} literal: {message}"),
        )]
    })
}

fn unsupported_fail_payload_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8004",
        "IR v0 can only lower fallible entry failure returns through a loaded static error constructor call with string code and message",
    )]
}
