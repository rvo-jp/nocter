use super::context::LoweringContext;
use super::expressions::lower_str_expression_to_location;
use crate::ast::{CallExpr, Expr, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{Instruction, StrLocation, StrValue};
use crate::literals::decode_string_literal_bytes;
use crate::resolve::{FunctionSignature, ResolveOutput, SymbolKind};
use crate::source::SourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ErrorPayload {
    instructions: Vec<Instruction>,
    code: StrValue,
    message: StrValue,
}

impl ErrorPayload {
    pub(super) fn into_return_instructions(self) -> Vec<Instruction> {
        let mut instructions = self.instructions;
        instructions.push(Instruction::ReturnFallibleFailure {
            code: self.code,
            message: self.message,
        });
        instructions
    }

    pub(super) fn into_store_instructions(
        self,
        code_destination: StrLocation,
        message_destination: StrLocation,
    ) -> Vec<Instruction> {
        let mut instructions = self.instructions;
        instructions.push(Instruction::SetStr {
            destination: code_destination,
            value: self.code,
        });
        instructions.push(Instruction::SetStr {
            destination: message_destination,
            value: self.message,
        });
        instructions
    }
}

pub(super) fn lower_error_payload(
    expression: &Expr,
    resolved: &ResolveOutput,
    root_source: SourceId,
    context: Option<&LoweringContext>,
) -> Result<Option<ErrorPayload>, Vec<Diagnostic>> {
    let call = match expression {
        Expr::Call(call) => call,
        Expr::Identifier(identifier) => {
            let Some(context) = context else {
                return Ok(None);
            };
            let Some(code) = context.error_code_location(&identifier.name) else {
                return Ok(None);
            };
            let Some(message) = context.error_message_location(&identifier.name) else {
                return Ok(None);
            };

            return Ok(Some(ErrorPayload {
                instructions: Vec::new(),
                code: StrValue::Location(code),
                message: StrValue::Location(message),
            }));
        }
        Expr::Group(group) => {
            return lower_error_payload(&group.expression, resolved, root_source, context);
        }
        _ => return Ok(None),
    };

    if !is_error_constructor_call(call, resolved, root_source) {
        return Ok(context.and_then(|context| context.error_payload_for_call(call)));
    }

    if call.arguments.len() != 2 {
        return Err(unsupported_fail_payload_diagnostic());
    };

    let mut instructions = Vec::new();
    let mut reserved_local_abi_words = 0;
    let code = lower_error_string_value(
        &call.arguments[0],
        "code",
        context,
        reserved_local_abi_words,
        &mut instructions,
    )?;
    if error_payload_value_uses_reserved_local(&code, context) {
        reserved_local_abi_words += 2;
    }
    let message = lower_error_string_value(
        &call.arguments[1],
        "message",
        context,
        reserved_local_abi_words,
        &mut instructions,
    )?;

    Ok(Some(ErrorPayload {
        instructions,
        code,
        message,
    }))
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
    reserved_local_abi_words: usize,
    instructions: &mut Vec<Instruction>,
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
        Expr::Group(group) => lower_error_string_value(
            &group.expression,
            field,
            context,
            reserved_local_abi_words,
            instructions,
        ),
        _ => {
            let Some(context) = context else {
                return Err(unsupported_fail_payload_diagnostic());
            };
            let destination_context =
                context.with_reserved_local_abi_words(reserved_local_abi_words);
            let destination = destination_context.next_str_local_location()?;
            let expression_context =
                context.with_reserved_local_abi_words(reserved_local_abi_words + 2);
            instructions.extend(lower_str_expression_to_location(
                expression,
                destination,
                &expression_context,
            )?);
            Ok(StrValue::Location(destination))
        }
    }
}

fn error_payload_value_uses_reserved_local(
    value: &StrValue,
    context: Option<&LoweringContext>,
) -> bool {
    let Some(context) = context else {
        return false;
    };
    let Ok(first_reserved) = context.first_temporary_local_index() else {
        return false;
    };
    matches!(value, StrValue::Location(StrLocation::Local(index)) if *index >= first_reserved)
}

fn unsupported_fail_payload_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8004",
        "IR cannot lower this fallible failure value because its code and message payload is not available",
    )]
}
