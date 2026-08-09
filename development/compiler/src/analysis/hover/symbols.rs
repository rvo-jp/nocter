use super::*;

pub(in crate::analysis::hover) fn hover_symbols_for_ast(
    text: &str,
    ast: &AstFile,
) -> Vec<HoverSymbol> {
    let mut symbols = Vec::new();
    for item in &ast.items {
        collect_item_hover_symbols(text, item, &mut symbols);
    }
    symbols
}

pub(in crate::analysis::hover) fn hover_symbols_for_file_analysis(
    text: &str,
    file: &FileAnalysis,
) -> Vec<HoverSymbol> {
    let mut symbols = hover_symbols_for_ast(text, &file.ast);
    apply_typecheck_hover_facts(text, &file.typecheck_facts, &mut symbols);
    symbols
}

pub(in crate::analysis::hover) fn apply_typecheck_hover_facts(
    text: &str,
    facts: &TypecheckFacts,
    symbols: &mut [HoverSymbol],
) {
    for symbol in symbols {
        let Some(ty) = facts.binding_type_label(symbol.target.declaration_span) else {
            continue;
        };
        if symbol.label.starts_with("capture ") {
            symbol.label = format!("{}: {ty}", symbol.label);
            continue;
        }
        let Some(kind) = binding_hover_label_kind(&symbol.label) else {
            continue;
        };
        let name = source_fragment(text, symbol.target.focus_span);
        symbol.label = format!("{kind} {name}: {ty}");
    }
}

pub(in crate::analysis::hover) fn binding_hover_label_kind(label: &str) -> Option<&'static str> {
    if label.starts_with("let ") {
        Some("let")
    } else if label.starts_with("var ") {
        Some("var")
    } else if label.starts_with("parameter ") {
        Some("parameter")
    } else if label.starts_with("catch ") {
        Some("catch")
    } else if label.starts_with("region ") {
        Some("region")
    } else {
        None
    }
}

pub(in crate::analysis::hover) fn collect_item_hover_symbols(
    text: &str,
    item: &Item,
    symbols: &mut Vec<HoverSymbol>,
) {
    match item {
        Item::Import(_) | Item::FromImport(_) => {}
        Item::Function(function) => {
            push_function_hover_symbol(text, function, symbols);
            collect_parameter_hover_symbols(&function.parameters.parameters, symbols);
            if let Some(body) = &function.body {
                collect_block_hover_symbols(text, body, symbols);
            }
        }
        Item::Test(test) => {
            push_hover_symbol(
                text,
                test.name_span,
                test.span.start,
                format!("test {}: void!", test.name),
                symbols,
            );
            collect_block_hover_symbols(text, &test.body, symbols);
        }
        Item::Primitive(primitive) => {
            push_primitive_hover_symbol(text, primitive, symbols);
            collect_parameter_hover_symbols(&primitive.parameters.parameters, symbols);
        }
        Item::TypeAlias(alias) => push_hover_symbol(
            text,
            alias.name_span,
            alias.span.start,
            crate::analysis::presentation::ast_type_alias_presentation(alias),
            symbols,
        ),
        Item::Struct(struct_) => collect_struct_hover_symbols(text, struct_, symbols),
        Item::Enum(enum_) => collect_enum_hover_symbols(text, enum_, symbols),
        Item::Interface(interface) => collect_interface_hover_symbols(text, interface, symbols),
        Item::Impl(impl_) => {
            for member in &impl_.members {
                match member {
                    ImplMember::AssociatedType(_) => {}
                    ImplMember::Method(method) => {
                        collect_method_hover_symbols(text, method, symbols)
                    }
                    ImplMember::Drop(drop_) => collect_drop_hover_symbols(text, drop_, symbols),
                }
            }
        }
        Item::Construct(construct) => {
            for (_, function) in construct.functions() {
                push_function_hover_symbol(text, function, symbols);
                collect_parameter_hover_symbols(&function.parameters.parameters, symbols);
                if let Some(body) = &function.body {
                    collect_block_hover_symbols(text, body, symbols);
                }
            }
            for (_, literal) in construct.literals() {
                collect_literal_hover_symbols(text, literal, symbols);
            }
        }
        Item::Coerce(coerce) => {
            for entry in &coerce.entries {
                push_hover_symbol(
                    text,
                    entry.as_span,
                    entry.span.start,
                    crate::analysis::presentation::ast_coercion_presentation(entry),
                    symbols,
                );
                let receiver = entry.receiver.implicit_parameter();
                collect_parameter_hover_symbols(std::slice::from_ref(&receiver), symbols);
                if let Some(body) = &entry.body {
                    collect_block_hover_symbols(text, body, symbols);
                }
            }
        }
    }
}

fn collect_literal_hover_symbols(
    text: &str,
    literal: &crate::ast::LiteralDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol(
        text,
        literal.shape_span,
        literal.span.start,
        crate::analysis::presentation::ast_literal_presentation(literal),
        symbols,
    );
    collect_parameter_hover_symbols(&literal.parameters.parameters, symbols);
    if let Some(capture) = &literal.capture {
        push_hover_symbol(
            text,
            capture.name_span,
            capture.span.start,
            format!(
                "literal pack {}: {}",
                capture.name,
                crate::ast::canonical_type_expr(&capture.element_type)
            ),
            symbols,
        );
    }
    if let Some(body) = &literal.body {
        collect_block_hover_symbols(text, body, symbols);
    }
}

pub(in crate::analysis::hover) fn collect_struct_hover_symbols(
    text: &str,
    struct_: &StructDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    let owner = generic_type_owner_name(
        &struct_.name,
        &struct_
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>(),
    );
    push_hover_symbol(
        text,
        struct_.name_span,
        struct_.span.start,
        crate::analysis::presentation::ast_struct_presentation(struct_),
        symbols,
    );
    for field in &struct_.fields {
        push_struct_field_hover_symbol(text, &owner, field, symbols);
    }
}

pub(in crate::analysis::hover) fn collect_enum_hover_symbols(
    text: &str,
    enum_: &EnumDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    let owner = generic_type_owner_name(
        &enum_.name,
        &enum_
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>(),
    );
    push_hover_symbol(
        text,
        enum_.name_span,
        enum_.span.start,
        crate::analysis::presentation::ast_enum_presentation(enum_),
        symbols,
    );
    for variant in &enum_.variants {
        push_hover_symbol(
            text,
            variant.name_span,
            variant.span.start,
            enum_variant_member_label(
                &owner,
                &variant.name,
                &crate::analysis::presentation::ast_parameter_labels(&variant.payload),
            ),
            symbols,
        );
        collect_parameter_hover_symbols(&variant.payload, symbols);
    }
}

pub(in crate::analysis::hover) fn collect_interface_hover_symbols(
    text: &str,
    interface: &InterfaceDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol(
        text,
        interface.name_span,
        interface.span.start,
        crate::analysis::presentation::ast_interface_presentation(interface),
        symbols,
    );
    for method in &interface.methods {
        collect_method_hover_symbols(text, method, symbols);
    }
}

pub(in crate::analysis::hover) fn collect_drop_hover_symbols(
    text: &str,
    drop_: &crate::ast::DropDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol(
        text,
        drop_.name_span,
        drop_.span.start,
        crate::analysis::presentation::ast_drop_presentation(drop_),
        symbols,
    );
    collect_parameter_hover_symbols(std::slice::from_ref(&drop_.binding), symbols);
    collect_block_hover_symbols(text, &drop_.body, symbols);
}

pub(in crate::analysis::hover) fn collect_method_hover_symbols(
    text: &str,
    method: &MethodDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol(
        text,
        method.name_span,
        method.span.start,
        crate::analysis::presentation::ast_method_presentation(method),
        symbols,
    );
    let receiver = method.receiver.implicit_parameter();
    collect_parameter_hover_symbols(std::slice::from_ref(&receiver), symbols);
    collect_parameter_hover_symbols(&method.parameters.parameters, symbols);
    if let Some(body) = &method.body {
        collect_block_hover_symbols(text, body, symbols);
    }
}

pub(in crate::analysis::hover) fn push_function_hover_symbol(
    text: &str,
    function: &FunctionDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol_for_declaration(
        text,
        function.member_name_span,
        function.name_span,
        function.span.start,
        crate::analysis::presentation::ast_function_presentation(function),
        symbols,
    );
}

pub(in crate::analysis::hover) fn push_primitive_hover_symbol(
    text: &str,
    primitive: &PrimitiveDecl,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol(
        text,
        primitive.name_span,
        primitive.span.start,
        crate::analysis::presentation::ast_primitive_presentation(primitive),
        symbols,
    );
}

pub(in crate::analysis::hover) fn push_struct_field_hover_symbol(
    text: &str,
    owner: &str,
    field: &StructField,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol(
        text,
        field.name_span,
        field.span.start,
        field_member_label(
            owner,
            &field.name,
            &crate::ast::canonical_type_expr(&field.ty),
        ),
        symbols,
    );
}

pub(in crate::analysis::hover) fn collect_parameter_hover_symbols(
    parameters: &[Parameter],
    symbols: &mut Vec<HoverSymbol>,
) {
    for parameter in parameters {
        push_hover_symbol_with_attach_start(
            parameter.name_span,
            parameter.name_span,
            parameter.span.start,
            format!(
                "parameter {}: {}",
                parameter.name,
                crate::ast::canonical_type_expr(&parameter.ty)
            ),
            symbols,
        );
    }
}

pub(in crate::analysis::hover) fn collect_block_hover_symbols(
    text: &str,
    block: &Block,
    symbols: &mut Vec<HoverSymbol>,
) {
    for statement in &block.statements {
        collect_statement_hover_symbols(text, statement, symbols);
    }
    if let Some(result) = &block.result {
        collect_expression_hover_symbols(text, result, symbols);
    }
}

pub(in crate::analysis::hover) fn collect_statement_hover_symbols(
    text: &str,
    statement: &Stmt,
    symbols: &mut Vec<HoverSymbol>,
) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression_hover_symbols(text, expression, symbols);
            }
        }
        Stmt::Binding(statement) => {
            push_binding_hover_symbol(text, statement, symbols);
            collect_expression_hover_symbols(text, &statement.initializer, symbols);
        }
        Stmt::Assignment(statement) => {
            collect_expression_hover_symbols(text, &statement.target, symbols);
            collect_expression_hover_symbols(text, &statement.value, symbols);
        }
        Stmt::If(statement) => {
            collect_expression_hover_symbols(text, &statement.condition, symbols);
            collect_block_hover_symbols(text, &statement.then_block, symbols);
            if let Some(block) = &statement.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Stmt::IfIs(statement) => {
            collect_expression_hover_symbols(text, &statement.expression, symbols);
            if let Some(payload) = statement
                .payload
                .as_ref()
                .and_then(|payload| payload.binding())
            {
                push_hover_symbol(
                    text,
                    payload.span,
                    statement.span.start,
                    format!("payload {}", payload.name),
                    symbols,
                );
            }
            collect_block_hover_symbols(text, &statement.then_block, symbols);
            if let Some(block) = &statement.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Stmt::Switch(statement) => {
            collect_expression_hover_symbols(text, &statement.expression, symbols);
            for arm in &statement.arms {
                collect_block_hover_symbols(text, &arm.body, symbols);
            }
            if let Some(arm) = &statement.wildcard_arm {
                collect_block_hover_symbols(text, &arm.body, symbols);
            }
        }
        Stmt::ForRange(statement) => {
            push_hover_symbol(
                text,
                statement.name_span,
                statement.span.start,
                format!("let {}", statement.name),
                symbols,
            );
            collect_expression_hover_symbols(text, &statement.start, symbols);
            collect_expression_hover_symbols(text, &statement.end, symbols);
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::CollectionFor(statement) => {
            push_hover_symbol(
                text,
                statement.name_span,
                statement.span.start,
                format!("let {}", statement.name),
                symbols,
            );
            collect_expression_hover_symbols(text, &statement.source, symbols);
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::LiteralPackFor(statement) => {
            push_hover_symbol(
                text,
                statement.name_span,
                statement.span.start,
                format!("let {}", statement.name),
                symbols,
            );
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::While(statement) => {
            collect_expression_hover_symbols(text, &statement.condition, symbols);
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::Loop(statement) => collect_block_hover_symbols(text, &statement.body, symbols),
        Stmt::Region(statement) => {
            push_hover_symbol(
                text,
                statement.name_span,
                statement.span.start,
                format!("region {}", statement.name),
                symbols,
            );
            collect_expression_hover_symbols(text, &statement.allocator, symbols);
            collect_block_hover_symbols(text, &statement.body, symbols);
        }
        Stmt::Drop(_) => {}
        Stmt::Expression(statement) => {
            collect_expression_hover_symbols(text, &statement.expression, symbols);
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

pub(in crate::analysis::hover) fn push_binding_hover_symbol(
    text: &str,
    statement: &BindingStmt,
    symbols: &mut Vec<HoverSymbol>,
) {
    let ty = statement
        .ty
        .as_ref()
        .map(|ty| format!(": {}", crate::ast::canonical_type_expr(ty)))
        .unwrap_or_default();
    push_hover_symbol(
        text,
        statement.name_span,
        statement.span.start,
        format!(
            "{} {}{}",
            binding_kind_label(statement.kind),
            statement.name,
            ty
        ),
        symbols,
    );
}

pub(in crate::analysis::hover) fn collect_expression_hover_symbols(
    text: &str,
    expression: &Expr,
    symbols: &mut Vec<HoverSymbol>,
) {
    match expression {
        Expr::Closure(expression) => {
            for capture in &expression.captures {
                push_hover_symbol(
                    text,
                    capture.name_span,
                    capture.span.start,
                    format!("capture {}{}", capture.mode.source_prefix(), capture.name),
                    symbols,
                );
            }
            for parameter in &expression.parameters {
                push_hover_symbol(
                    text,
                    parameter.name_span,
                    parameter.span.start,
                    format!("parameter {}", parameter.name),
                    symbols,
                );
            }
            collect_block_hover_symbols(text, &expression.body, symbols);
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_hover_symbols(text, element, symbols);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                collect_expression_hover_symbols(text, element, symbols);
            }
            if let Some(using) = &expression.using {
                collect_expression_hover_symbols(text, &using.allocator, symbols);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if let Some(using) = &expression.using {
                collect_expression_hover_symbols(text, &using.allocator, symbols);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_expression_hover_symbols(text, &field.value, symbols);
            }
        }
        Expr::Propagate(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Force(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Catch(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
            push_hover_symbol(
                text,
                expression.error_span,
                expression.catch_span.start,
                format!("catch {}", expression.error_name),
                symbols,
            );
            collect_block_hover_symbols(text, &expression.catch_block, symbols);
        }
        Expr::Borrow(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Unary(expression) => {
            collect_expression_hover_symbols(text, &expression.operand, symbols)
        }
        Expr::Binary(expression) => {
            collect_expression_hover_symbols(text, &expression.left, symbols);
            collect_expression_hover_symbols(text, &expression.right, symbols);
        }
        Expr::TypeConversion(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::Call(expression) => {
            collect_expression_hover_symbols(text, &expression.callee, symbols);
            for argument in &expression.arguments {
                collect_expression_hover_symbols(text, argument, symbols);
            }
        }
        Expr::Member(expression) => {
            collect_expression_hover_symbols(text, &expression.object, symbols)
        }
        Expr::Index(expression) => {
            collect_expression_hover_symbols(text, &expression.object, symbols);
            collect_expression_hover_symbols(text, &expression.index, symbols);
        }
        Expr::Group(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    collect_expression_hover_symbols(text, &part.expression, symbols);
                }
            }
        }
        Expr::Otherwise(expression) => {
            collect_expression_hover_symbols(text, &expression.value, symbols);
            collect_block_hover_symbols(text, &expression.fallback, symbols);
        }
        Expr::If(expression) => {
            collect_expression_hover_symbols(text, &expression.condition, symbols);
            collect_block_hover_symbols(text, &expression.then_block, symbols);
            if let Some(block) = &expression.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Expr::IfIs(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
            if let Some(payload) = expression
                .payload
                .as_ref()
                .and_then(|payload| payload.binding())
            {
                push_hover_symbol(
                    text,
                    payload.span,
                    expression.span.start,
                    format!("payload {}", payload.name),
                    symbols,
                );
            }
            collect_block_hover_symbols(text, &expression.then_block, symbols);
            if let Some(block) = &expression.else_block {
                collect_block_hover_symbols(text, block, symbols);
            }
        }
        Expr::Match(expression) => {
            collect_expression_hover_symbols(text, &expression.expression, symbols);
            for arm in &expression.arms {
                collect_block_hover_symbols(text, &arm.body, symbols);
            }
            if let Some(arm) = &expression.wildcard_arm {
                collect_block_hover_symbols(text, &arm.body, symbols);
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

pub(in crate::analysis::hover) fn push_hover_symbol(
    text: &str,
    name_span: ByteSpan,
    declaration_start: usize,
    label: String,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol_for_declaration(
        text,
        name_span,
        name_span,
        declaration_start,
        label,
        symbols,
    );
}

pub(in crate::analysis::hover) fn push_hover_symbol_for_declaration(
    text: &str,
    focus_span: ByteSpan,
    declaration_span: ByteSpan,
    declaration_start: usize,
    label: String,
    symbols: &mut Vec<HoverSymbol>,
) {
    push_hover_symbol_with_attach_start(
        focus_span,
        declaration_span,
        declaration_line_start(text, declaration_start),
        label,
        symbols,
    );
}

pub(in crate::analysis::hover) fn push_hover_symbol_with_attach_start(
    focus_span: ByteSpan,
    declaration_span: ByteSpan,
    attach_start: usize,
    label: String,
    symbols: &mut Vec<HoverSymbol>,
) {
    symbols.push(HoverSymbol {
        target: crate::analysis::editor_targets::SourceTarget::new(focus_span, declaration_span),
        attach_start,
        label,
    });
}
