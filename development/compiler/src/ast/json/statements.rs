use super::*;

impl Block {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        self.to_json_with_kind(sources, "block")
    }

    pub(super) fn to_json_with_kind(&self, sources: &SourceMap, kind: &str) -> JsonAstNode {
        let mut children = self
            .statements
            .iter()
            .map(|statement| statement.to_json(sources))
            .collect::<Vec<_>>();
        if let Some(result) = &self.result {
            children.push(JsonAstNode::new(
                "block_result",
                json_span(sources, result.span()),
                vec![result.to_json(sources)],
            ));
        }

        JsonAstNode::new(kind, json_span(sources, self.span), children)
    }
}

impl Stmt {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            Stmt::Import(item) => JsonAstNode::with_value(
                "use_namespace_statement",
                item.path.value.clone(),
                json_span(sources, item.span),
                vec![item.path.to_json(sources), item.alias.to_json(sources)],
            ),
            Stmt::FromImport(item) => {
                let mut children = vec![item.path.to_json(sources)];
                children.extend(item.names.iter().map(|name| name.to_json(sources)));
                JsonAstNode::with_value(
                    "use_names_statement",
                    item.path.value.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Stmt::Return(statement) => JsonAstNode::new(
                "return_statement",
                json_span(sources, statement.span),
                statement
                    .expression
                    .iter()
                    .map(|expression| expression.to_json(sources))
                    .collect(),
            ),
            Stmt::Binding(statement) => {
                let mut children = Vec::new();
                if let Some(ty) = &statement.ty {
                    children.push(ty.to_json(sources));
                }
                children.push(statement.initializer.to_json(sources));
                JsonAstNode::with_value(
                    match statement.kind {
                        BindingKind::Let => "let_statement",
                        BindingKind::Var => "var_statement",
                    },
                    statement.name.clone(),
                    json_span(sources, statement.span),
                    children,
                )
            }
            Stmt::Assignment(statement) => JsonAstNode::with_value(
                "assignment_statement",
                assignment_operator_name(statement.operator).to_string(),
                json_span(sources, statement.span),
                vec![
                    statement.target.to_json(sources),
                    statement.value.to_json(sources),
                ],
            ),
            Stmt::If(statement) => {
                let mut children = vec![
                    statement.condition.to_json(sources),
                    statement
                        .then_block
                        .to_json_with_kind(sources, "then_block"),
                ];
                if let Some(else_block) = &statement.else_block {
                    children.push(else_block.to_json_with_kind(sources, "else_block"));
                }
                JsonAstNode::new("if_statement", json_span(sources, statement.span), children)
            }
            Stmt::IfIs(statement) => {
                let mut pattern_children = Vec::new();
                if let Some(payload) = &statement.payload {
                    pattern_children.push(payload_pattern_json(
                        payload,
                        sources,
                        "if_is_payload_binding",
                        "if_is_payload_discard",
                    ));
                }

                let mut children = vec![
                    statement.expression.to_json(sources),
                    JsonAstNode::with_value(
                        "if_is_pattern",
                        if_is_pattern_value(statement),
                        json_span(sources, statement.pattern_span),
                        pattern_children,
                    ),
                    statement
                        .then_block
                        .to_json_with_kind(sources, "then_block"),
                ];
                if let Some(else_block) = &statement.else_block {
                    children.push(else_block.to_json_with_kind(sources, "else_block"));
                }
                JsonAstNode::new(
                    "if_is_statement",
                    json_span(sources, statement.span),
                    children,
                )
            }
            Stmt::Switch(statement) => {
                let mut children = vec![statement.expression.to_json(sources)];
                children.extend(statement.arms.iter().map(|arm| arm.to_json(sources)));
                if let Some(wildcard_arm) = &statement.wildcard_arm {
                    children.push(wildcard_arm.to_json(sources));
                }
                JsonAstNode::new(
                    "match_statement",
                    json_span(sources, statement.span),
                    children,
                )
            }
            Stmt::ForRange(statement) => JsonAstNode::with_value(
                "for_range_statement",
                statement.name.clone(),
                json_span(sources, statement.span),
                vec![
                    JsonAstNode::with_value(
                        "for_binding",
                        statement.name.clone(),
                        json_span(sources, statement.name_span),
                        Vec::new(),
                    ),
                    statement.start.to_json(sources),
                    statement.end.to_json(sources),
                    statement.body.to_json_with_kind(sources, "body"),
                ],
            ),
            Stmt::While(statement) => JsonAstNode::new(
                "while_statement",
                json_span(sources, statement.span),
                vec![
                    statement.condition.to_json(sources),
                    statement.body.to_json_with_kind(sources, "body"),
                ],
            ),
            Stmt::Loop(statement) => JsonAstNode::new(
                "loop_statement",
                json_span(sources, statement.span),
                vec![statement.body.to_json_with_kind(sources, "body")],
            ),
            Stmt::Region(statement) => JsonAstNode::with_value(
                "region_statement",
                statement.name.clone(),
                json_span(sources, statement.span),
                vec![
                    JsonAstNode::with_value(
                        "region_binding",
                        statement.name.clone(),
                        json_span(sources, statement.name_span),
                        Vec::new(),
                    ),
                    statement.allocator.to_json(sources),
                    statement.body.to_json_with_kind(sources, "body"),
                ],
            ),
            Stmt::Break(statement) => JsonAstNode::new(
                "break_statement",
                json_span(sources, statement.span),
                Vec::new(),
            ),
            Stmt::Continue(statement) => JsonAstNode::new(
                "continue_statement",
                json_span(sources, statement.span),
                Vec::new(),
            ),
            Stmt::Drop(statement) => JsonAstNode::with_value(
                "drop_statement",
                statement.name.clone(),
                json_span(sources, statement.span),
                Vec::new(),
            ),
            Stmt::Expression(statement) => JsonAstNode::new(
                "expression_statement",
                json_span(sources, statement.span),
                vec![statement.expression.to_json(sources)],
            ),
        }
    }
}

fn assignment_operator_name(operator: crate::ast::AssignmentOperator) -> &'static str {
    match operator {
        crate::ast::AssignmentOperator::Assign => "=",
        crate::ast::AssignmentOperator::AddAssign => "+=",
        crate::ast::AssignmentOperator::SubtractAssign => "-=",
        crate::ast::AssignmentOperator::MultiplyAssign => "*=",
        crate::ast::AssignmentOperator::DivideAssign => "/=",
        crate::ast::AssignmentOperator::RemainderAssign => "%=",
    }
}

impl SwitchArm {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let mut children = vec![
            JsonAstNode::with_value(
                "match_pattern",
                format!("{}.{}", self.enum_name, self.variant_name),
                json_span(sources, self.span),
                Vec::new(),
            ),
            self.body.to_json_with_kind(sources, "body"),
        ];
        if let Some(payload) = &self.payload {
            children.insert(
                1,
                payload_pattern_json(
                    payload,
                    sources,
                    "match_payload_binding",
                    "match_payload_discard",
                ),
            );
        }

        JsonAstNode::with_value(
            "match_arm",
            format!("{}.{}", self.enum_name, self.variant_name),
            json_span(sources, self.span),
            children,
        )
    }
}

impl SwitchWildcardArm {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::new(
            "match_wildcard_arm",
            json_span(sources, self.span),
            vec![self.body.to_json_with_kind(sources, "body")],
        )
    }
}

pub(super) fn payload_pattern_json(
    payload: &SwitchPayloadPattern,
    sources: &SourceMap,
    binding_kind: &str,
    discard_kind: &str,
) -> JsonAstNode {
    match payload {
        SwitchPayloadPattern::Binding(binding) => JsonAstNode::with_value(
            binding_kind,
            binding.name.clone(),
            json_span(sources, binding.span),
            Vec::new(),
        ),
        SwitchPayloadPattern::Discard(discard) => JsonAstNode::with_value(
            discard_kind,
            "_",
            json_span(sources, discard.span),
            Vec::new(),
        ),
    }
}

pub(super) fn if_is_pattern_value(statement: &IfIsStmt) -> String {
    format!("{}.{}", statement.enum_name, statement.variant_name)
}
