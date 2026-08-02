use super::*;

impl Expr {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            Expr::Identifier(expression) => JsonAstNode::with_value(
                "identifier",
                expression.name.clone(),
                json_span(sources, expression.span),
                Vec::new(),
            ),
            Expr::IntegerLiteral(expression) => JsonAstNode::with_value(
                "integer_literal",
                expression.value.clone(),
                json_span(sources, expression.span),
                Vec::new(),
            ),
            Expr::ByteLiteral(expression) => JsonAstNode::with_value(
                "byte_literal",
                expression.value.clone(),
                json_span(sources, expression.span),
                Vec::new(),
            ),
            Expr::StringLiteral(expression) => JsonAstNode::with_value(
                "string_literal",
                expression.value.clone(),
                json_span(sources, expression.span),
                Vec::new(),
            ),
            Expr::InterpolatedString(expression) => JsonAstNode::with_value(
                "interpolated_string",
                expression.value.clone(),
                json_span(sources, expression.span),
                expression
                    .parts
                    .iter()
                    .map(|part| match part {
                        InterpolatedStringPart::Text(part) => JsonAstNode::with_value(
                            "interpolated_string_text",
                            part.value.clone(),
                            json_span(sources, part.span),
                            Vec::new(),
                        ),
                        InterpolatedStringPart::Expression(part) => JsonAstNode::new(
                            "string_interpolation",
                            json_span(sources, part.span),
                            vec![part.expression.to_json(sources)],
                        ),
                    })
                    .collect(),
            ),
            Expr::BoolLiteral(expression) => JsonAstNode::with_value(
                "bool_literal",
                expression.value.clone(),
                json_span(sources, expression.span),
                Vec::new(),
            ),
            Expr::NoneLiteral(expression) => JsonAstNode::with_value(
                "none_literal",
                expression.value.clone(),
                json_span(sources, expression.span),
                Vec::new(),
            ),
            Expr::ArrayLiteral(expression) => JsonAstNode::new(
                "array_literal",
                json_span(sources, expression.span),
                expression
                    .elements
                    .iter()
                    .map(|element| element.to_json(sources))
                    .collect(),
            ),
            Expr::TypedSequenceLiteral(expression) => {
                let mut children = vec![
                    expression.target.to_json(sources),
                    JsonAstNode::new(
                        "typed_literal_element_list",
                        json_span(sources, expression.elements_span),
                        expression
                            .elements
                            .iter()
                            .map(|element| element.to_json(sources))
                            .collect(),
                    ),
                ];
                if let Some(using) = &expression.using {
                    children.push(JsonAstNode::new(
                        "literal_context_override",
                        json_span(sources, using.span),
                        vec![using.allocator.to_json(sources)],
                    ));
                }
                JsonAstNode::new(
                    "typed_sequence_literal",
                    json_span(sources, expression.span),
                    children,
                )
            }
            Expr::TypedStringLiteral(expression) => {
                let mut children = vec![
                    expression.target.to_json(sources),
                    JsonAstNode::with_value(
                        "string_literal",
                        expression.text.value.clone(),
                        json_span(sources, expression.text.span),
                        Vec::new(),
                    ),
                ];
                if let Some(using) = &expression.using {
                    children.push(JsonAstNode::new(
                        "literal_context_override",
                        json_span(sources, using.span),
                        vec![using.allocator.to_json(sources)],
                    ));
                }
                JsonAstNode::new(
                    "typed_string_literal",
                    json_span(sources, expression.span),
                    children,
                )
            }
            Expr::StructLiteral(expression) => JsonAstNode::new(
                "struct_literal",
                json_span(sources, expression.span),
                vec![
                    expression.ty.to_json(sources),
                    JsonAstNode::new(
                        "struct_literal_field_list",
                        json_span(sources, expression.fields_span),
                        expression
                            .fields
                            .iter()
                            .map(|field| field.to_json(sources))
                            .collect(),
                    ),
                ],
            ),
            Expr::Propagate(expression) => JsonAstNode::new(
                "propagation_expression",
                json_span(sources, expression.span),
                vec![expression.expression.to_json(sources)],
            )
            .with_operator_span(json_span(sources, expression.operator_span)),
            Expr::Force(expression) => JsonAstNode::new(
                "force_unwrap_expression",
                json_span(sources, expression.span),
                vec![expression.expression.to_json(sources)],
            )
            .with_operator_span(json_span(sources, expression.operator_span)),
            Expr::Catch(expression) => JsonAstNode::new(
                "fallible_catch_expression",
                json_span(sources, expression.span),
                vec![
                    expression.expression.to_json(sources),
                    JsonAstNode::with_value(
                        "catch_binding",
                        expression.error_name.clone(),
                        json_span(sources, expression.error_span),
                        Vec::new(),
                    ),
                    expression.catch_block.to_json(sources),
                ],
            )
            .with_operator_span(json_span(sources, expression.catch_span)),
            Expr::Borrow(expression) => JsonAstNode::with_value(
                "borrow_expression",
                if expression.is_readwrite { "&+" } else { "&" },
                json_span(sources, expression.span),
                vec![expression.expression.to_json(sources)],
            )
            .with_operator_span(json_span(sources, expression.operator_span)),
            Expr::Unary(expression) => JsonAstNode::with_value(
                "unary_expression",
                expression.operator.spelling(),
                json_span(sources, expression.span),
                vec![expression.operand.to_json(sources)],
            )
            .with_operator_span(json_span(sources, expression.operator_span)),
            Expr::Binary(expression) => JsonAstNode::with_value(
                "binary_expression",
                expression.operator.spelling(),
                json_span(sources, expression.span),
                vec![
                    expression.left.to_json(sources),
                    expression.right.to_json(sources),
                ],
            )
            .with_operator_span(json_span(sources, expression.operator_span)),
            Expr::TypeConversion(expression) => JsonAstNode::new(
                "type_conversion_expression",
                json_span(sources, expression.span),
                vec![
                    expression.expression.to_json(sources),
                    expression.ty.to_json(sources),
                ],
            )
            .with_operator_span(json_span(sources, expression.as_span)),
            Expr::Call(expression) => JsonAstNode::new(
                "call_expression",
                json_span(sources, expression.span),
                vec![
                    expression.callee.to_json(sources),
                    JsonAstNode::new(
                        "argument_list",
                        json_span(sources, expression.arguments_span),
                        expression
                            .arguments
                            .iter()
                            .map(|argument| argument.to_json(sources))
                            .collect(),
                    ),
                ],
            ),
            Expr::Member(expression) => JsonAstNode::with_value(
                "member_expression",
                expression.member.clone(),
                json_span(sources, expression.span),
                vec![expression.object.to_json(sources)],
            ),
            Expr::Index(expression) => JsonAstNode::new(
                "index_expression",
                json_span(sources, expression.span),
                vec![
                    expression.object.to_json(sources),
                    expression.index.to_json(sources),
                ],
            ),
            Expr::Group(expression) => JsonAstNode::new(
                "group_expression",
                json_span(sources, expression.span),
                vec![expression.expression.to_json(sources)],
            ),
            Expr::Otherwise(expression) => JsonAstNode::new(
                "otherwise_expression",
                json_span(sources, expression.span),
                vec![
                    expression.value.to_json(sources),
                    expression
                        .fallback
                        .to_json_with_kind(sources, "fallback_block"),
                ],
            )
            .with_operator_span(json_span(sources, expression.keyword_span)),
            Expr::If(expression) => {
                let mut children = vec![
                    expression.condition.to_json(sources),
                    expression
                        .then_block
                        .to_json_with_kind(sources, "then_block"),
                ];
                if let Some(else_block) = &expression.else_block {
                    children.push(else_block.to_json_with_kind(sources, "else_block"));
                }
                JsonAstNode::new(
                    "if_expression",
                    json_span(sources, expression.span),
                    children,
                )
            }
            Expr::IfIs(expression) => {
                let mut pattern_children = Vec::new();
                if let Some(payload) = &expression.payload {
                    pattern_children.push(payload_pattern_json(
                        payload,
                        sources,
                        "if_is_payload_binding",
                        "if_is_payload_discard",
                    ));
                }

                let mut children = vec![
                    expression.expression.to_json(sources),
                    JsonAstNode::with_value(
                        "if_is_pattern",
                        if_is_pattern_value(expression),
                        json_span(sources, expression.pattern_span),
                        pattern_children,
                    ),
                    expression
                        .then_block
                        .to_json_with_kind(sources, "then_block"),
                ];
                if let Some(else_block) = &expression.else_block {
                    children.push(else_block.to_json_with_kind(sources, "else_block"));
                }
                JsonAstNode::new(
                    "if_is_expression",
                    json_span(sources, expression.span),
                    children,
                )
            }
            Expr::Match(expression) => {
                let mut children = vec![expression.expression.to_json(sources)];
                children.extend(expression.arms.iter().map(|arm| arm.to_json(sources)));
                if let Some(wildcard_arm) = &expression.wildcard_arm {
                    children.push(wildcard_arm.to_json(sources));
                }
                JsonAstNode::new(
                    "match_expression",
                    json_span(sources, expression.span),
                    children,
                )
            }
        }
    }
}
