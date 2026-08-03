use super::*;

pub(super) fn function_declaration_hover_label(
    function: &crate::ast::FunctionDecl,
    resolved: &ResolveOutput,
) -> String {
    let self_type = function_self_type(function, resolved);
    let mut label = format!(
        "func {}{}({}): {}",
        function.name,
        generic_parameters_label(&function.generics, resolved, self_type.as_ref()),
        parameters_label(
            &function.parameters.parameters,
            resolved,
            self_type.as_ref()
        ),
        type_label(&function.return_type, resolved, self_type.as_ref())
    );
    append_result_provenance(&mut label, function.result_provenance.as_ref());
    label
}

pub(super) fn primitive_declaration_hover_label(
    primitive: &crate::ast::PrimitiveDecl,
    resolved: &ResolveOutput,
) -> String {
    let mut label = format!(
        "primitive {}{}({}): {}",
        primitive.name,
        generic_parameters_label(&primitive.generics, resolved, None),
        parameters_label(&primitive.parameters.parameters, resolved, None),
        type_label(&primitive.return_type, resolved, None)
    );
    append_result_provenance(&mut label, primitive.result_provenance.as_ref());
    label
}

pub(super) fn literal_declaration_hover_label(
    literal: &crate::ast::LiteralDecl,
    resolved: &ResolveOutput,
) -> String {
    let environment = environment_for_literal(literal, resolved);
    let self_type = environment.self_type();
    let shape = match literal.shape {
        crate::ast::LiteralShape::Sequence => "[]",
        crate::ast::LiteralShape::String => "\"\"",
    };
    let parameters = literal.capture.as_ref().map_or_else(
        || parameters_label(&literal.parameters.parameters, resolved, self_type),
        |capture| {
            format!(
                "...{}: {}",
                capture.name,
                type_label(&capture.element_type, resolved, self_type)
            )
        },
    );

    let mut label = format!(
        "literal {} {shape}({parameters}): {}",
        type_label(&literal.target, resolved, self_type),
        type_label(&literal.return_type, resolved, self_type)
    );
    append_result_provenance(&mut label, literal.result_provenance.as_ref());
    label
}

pub(super) fn type_alias_declaration_hover_label(
    alias: &TypeAliasDecl,
    resolved: &ResolveOutput,
) -> String {
    format!(
        "type {}{} = {}",
        alias.name,
        generic_parameters_label(&alias.generics, resolved, None),
        type_label(&alias.target, resolved, None)
    )
}

pub(super) fn struct_declaration_hover_label(
    struct_: &StructDecl,
    resolved: &ResolveOutput,
) -> String {
    let copy_prefix = if struct_.is_copy { "copy " } else { "" };
    format!(
        "{copy_prefix}struct {}{}",
        struct_.name,
        generic_parameters_label(&struct_.generics, resolved, None)
    )
}

pub(super) fn struct_field_declaration_hover_label(
    owner: &StructDecl,
    field: &StructField,
    resolved: &ResolveOutput,
) -> String {
    field_member_label(
        &declared_member_owner_label(&owner.name, &owner.generics),
        &field.name,
        &type_label(&field.ty, resolved, None),
    )
}

pub(super) fn enum_declaration_hover_label(enum_: &EnumDecl, resolved: &ResolveOutput) -> String {
    format!(
        "enum {}{}",
        enum_.name,
        generic_parameters_label(&enum_.generics, resolved, None)
    )
}

fn declared_member_owner_label(name: &str, generics: &GenericParamList) -> String {
    generic_type_owner_name(
        name,
        &generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>(),
    )
}

pub(super) fn enum_variant_declaration_hover_label(
    owner: &EnumDecl,
    variant: &EnumVariant,
    resolved: &ResolveOutput,
) -> String {
    enum_variant_member_label(
        &declared_member_owner_label(&owner.name, &owner.generics),
        &variant.name,
        &parameter_labels(&variant.payload, resolved, None),
    )
}

pub(super) fn enum_variant_signature_hover_label(
    owner: &TypeSymbol,
    variant: &crate::resolve::EnumVariantSignature,
    resolved: &ResolveOutput,
) -> String {
    enum_variant_member_label(
        type_owner_hover_label(owner, resolved),
        &variant.name,
        &parameter_signature_labels(&variant.payload, resolved, None),
    )
}

pub(super) fn method_declaration_hover_label(
    method: &MethodDecl,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    let mut label = format!(
        "method {}.{}({}): {}",
        method_receiver_label(&method.receiver, resolved, self_type),
        method.name,
        parameters_label(&method.parameters.parameters, resolved, self_type),
        type_label(&method.return_type, resolved, self_type)
    );
    append_result_provenance(&mut label, method.result_provenance.as_ref());
    label
}

pub(super) fn drop_declaration_hover_label(
    drop_: &crate::ast::DropDecl,
    resolved: &ResolveOutput,
    self_type: &Type,
) -> String {
    format!(
        "drop {}",
        parameter_receiver_label(&drop_.binding, resolved, Some(self_type))
    )
}

pub(super) fn associated_function_signature_hover_label(
    owner: &TypeSymbol,
    function: &AssociatedFunctionSignature,
    resolved: &ResolveOutput,
) -> String {
    let self_type = Type::Named(owner.canonical_name.clone());
    let name = format!(
        "{}.{}",
        type_owner_hover_label(owner, resolved),
        function.name
    );
    function_signature_hover_label(
        "func",
        &name,
        &function.signature,
        resolved,
        Some(&self_type),
    )
}

pub(super) fn method_signature_hover_label(
    method: &MethodSignature,
    owner: &TypeSymbol,
    resolved: &ResolveOutput,
) -> String {
    let self_type = Type::Named(owner.canonical_name.clone());
    let mut label = format!(
        "method {}.{}({}): {}",
        method_receiver_label(&method.receiver, resolved, Some(&self_type)),
        method.name,
        parameter_signatures_label(&method.signature.parameters, resolved, Some(&self_type)),
        type_label(&method.signature.return_type, resolved, Some(&self_type))
    );
    append_result_provenance(&mut label, method.signature.result_provenance.as_ref());
    label
}

pub(super) fn method_receiver_label(
    receiver: &crate::ast::MethodReceiver,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    format!(
        "{}{}",
        receiver.mode.source_prefix(),
        receiver_owner_label(resolved, self_type)
    )
}

pub(super) fn parameter_receiver_label(
    receiver: &Parameter,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    match self_receiver_prefix(&receiver.ty) {
        Some(prefix) => format!("{prefix}{}", receiver.name),
        None => format!(
            "{}: {}",
            receiver.name,
            type_label(&receiver.ty, resolved, self_type)
        ),
    }
}

fn receiver_owner_label(resolved: &ResolveOutput, self_type: Option<&Type>) -> String {
    self_type
        .map(|self_type| type_hover_label(self_type, resolved))
        .unwrap_or_else(|| "Self".to_string())
}

pub(super) fn self_receiver_prefix(ty: &TypeExpr) -> Option<&'static str> {
    match method_receiver_kind(ty)? {
        TypecheckMethodReceiverKind::Owned => Some(""),
        TypecheckMethodReceiverKind::ReadonlyBorrow => Some("&"),
        TypecheckMethodReceiverKind::ReadwriteBorrow => Some("&+"),
    }
}

pub(super) fn method_receiver_kind(ty: &TypeExpr) -> Option<TypecheckMethodReceiverKind> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "Self" => {
            Some(TypecheckMethodReceiverKind::Owned)
        }
        TypeExpr::Borrow(borrow) => match borrow.inner.as_ref() {
            TypeExpr::Reference(reference) if reference.name == "Self" => {
                Some(if borrow.is_readwrite {
                    TypecheckMethodReceiverKind::ReadwriteBorrow
                } else {
                    TypecheckMethodReceiverKind::ReadonlyBorrow
                })
            }
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn function_signature_hover_label(
    kind: &str,
    name: &str,
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    let mut label = format!(
        "{kind} {name}({}): {}",
        parameter_signatures_label(&signature.parameters, resolved, self_type),
        type_label(&signature.return_type, resolved, self_type)
    );
    append_result_provenance(&mut label, signature.result_provenance.as_ref());
    label
}

fn append_result_provenance(
    label: &mut String,
    clause: Option<&crate::ast::ResultProvenanceClause>,
) {
    let Some(clause) = clause else {
        return;
    };
    label.push_str(" from ");
    label.push_str(
        &clause
            .origins
            .iter()
            .map(|origin| origin.kind.source_label())
            .collect::<Vec<_>>()
            .join(" | "),
    );
}

pub(super) fn generic_parameters_label(
    generics: &GenericParamList,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    if generics.parameters.is_empty() {
        return String::new();
    }

    let parameters = generics
        .parameters
        .iter()
        .map(|parameter| match &parameter.bound {
            Some(bound) => format!(
                "{}: {}",
                parameter.name,
                type_label(bound, resolved, self_type)
            ),
            None => parameter.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("<{parameters}>")
}

pub(super) fn parameters_label(
    parameters: &[Parameter],
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    parameter_labels(parameters, resolved, self_type).join(", ")
}

fn parameter_labels(
    parameters: &[Parameter],
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Vec<String> {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                type_label(&parameter.ty, resolved, self_type)
            )
        })
        .collect()
}

pub(super) fn parameter_signatures_label(
    parameters: &[ParameterSignature],
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    parameter_signature_labels(parameters, resolved, self_type).join(", ")
}

fn parameter_signature_labels(
    parameters: &[ParameterSignature],
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> Vec<String> {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                parameter_signature_type_label(parameter, resolved, self_type)
            )
        })
        .collect()
}

pub(super) fn parameter_signature_type_label(
    parameter: &ParameterSignature,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    type_label(&parameter.ty, resolved, self_type)
}

pub(super) fn type_label(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
) -> String {
    type_hover_label(
        &type_expr_to_type_with_self_type(ty, resolved, self_type),
        resolved,
    )
}

pub(super) fn type_hover_label(ty: &Type, resolved: &ResolveOutput) -> String {
    match ty {
        Type::I32 => "i32".to_string(),
        Type::Primitive(name) => name.clone(),
        Type::StrData => "str".to_string(),
        Type::Str => "&str".to_string(),
        Type::Error => "error".to_string(),
        Type::Void => "void".to_string(),
        Type::Never => "never".to_string(),
        Type::None => "none".to_string(),
        Type::ArrayData { element } => format!("[{}]", type_hover_label(element, resolved)),
        Type::View {
            is_readwrite: true,
            element,
        } => format!("&+[{}]", type_hover_label(element, resolved)),
        Type::View {
            is_readwrite: false,
            element,
        } => format!("&[{}]", type_hover_label(element, resolved)),
        Type::Array { element, length } => {
            format!("[{}; {}]", type_hover_label(element, resolved), length)
        }
        Type::Pointer(inner) => format!("*{}", type_hover_label(inner, resolved)),
        Type::Optional(inner) => format!("{}?", suffix_operand_hover_label(inner, resolved)),
        Type::Fallible { success, .. } => {
            format!("{}!", suffix_operand_hover_label(success, resolved))
        }
        Type::Named(name) => {
            if let Some(inner) = name.strip_prefix("&+") {
                format!(
                    "&+{}",
                    type_hover_label(&simple_type_from_display_name(inner), resolved)
                )
            } else if let Some(inner) = name.strip_prefix('&') {
                format!(
                    "&{}",
                    type_hover_label(&simple_type_from_display_name(inner), resolved)
                )
            } else {
                display_type_name(name, resolved).to_string()
            }
        }
        Type::Generic { name, arguments } => {
            let name = display_type_name(name, resolved);
            let arguments = arguments
                .iter()
                .map(|argument| type_hover_label(argument, resolved))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}<{arguments}>")
        }
        Type::Parameter(name) => name.clone(),
        Type::Unresolved(name) => name.clone(),
        Type::Unknown => "<unknown>".to_string(),
    }
}

fn suffix_operand_hover_label(ty: &Type, resolved: &ResolveOutput) -> String {
    let label = type_hover_label(ty, resolved);
    if matches!(ty, Type::Str | Type::View { .. })
        || matches!(ty, Type::Named(name) if name.starts_with('&'))
    {
        format!("({label})")
    } else {
        label
    }
}

pub(super) fn type_owner_hover_label<'a>(
    owner: &'a TypeSymbol,
    resolved: &'a ResolveOutput,
) -> &'a str {
    display_type_name(&owner.canonical_name, resolved)
}

pub(super) fn display_type_name<'a>(
    canonical_name: &'a str,
    resolved: &'a ResolveOutput,
) -> &'a str {
    visible_type_name(canonical_name, resolved).unwrap_or_else(|| short_type_name(canonical_name))
}

pub(super) fn short_type_name(canonical_name: &str) -> &str {
    canonical_name
        .rsplit_once('.')
        .map(|(_, name)| name)
        .unwrap_or(canonical_name)
}

pub(super) fn visible_type_name<'a>(
    canonical_name: &str,
    resolved: &'a ResolveOutput,
) -> Option<&'a str> {
    resolved
        .symbols
        .symbols()
        .filter_map(|symbol| match &symbol.kind {
            SymbolKind::Type(type_symbol)
                if type_symbol.canonical_name == canonical_name
                    && symbol.name != canonical_name =>
            {
                Some(symbol.name.as_str())
            }
            SymbolKind::Function(_)
            | SymbolKind::Primitive(_)
            | SymbolKind::Type(_)
            | SymbolKind::Imported(_) => None,
        })
        .min_by_key(|name| name.len())
}
