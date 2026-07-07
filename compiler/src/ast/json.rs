use super::*;
use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, JsonSpan, SourceMap};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonAstNode {
    pub kind: String,
    pub span: Option<JsonSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_span: Option<JsonSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub items: Vec<JsonAstNode>,
}

impl JsonAstNode {
    pub fn new(kind: impl Into<String>, span: Option<JsonSpan>, items: Vec<JsonAstNode>) -> Self {
        Self {
            kind: kind.into(),
            span,
            operator_span: None,
            value: None,
            items,
        }
    }

    pub fn with_value(
        kind: impl Into<String>,
        value: impl Into<String>,
        span: Option<JsonSpan>,
        items: Vec<JsonAstNode>,
    ) -> Self {
        Self {
            kind: kind.into(),
            span,
            operator_span: None,
            value: Some(value.into()),
            items,
        }
    }

    pub fn with_operator_span(mut self, operator_span: Option<JsonSpan>) -> Self {
        self.operator_span = operator_span;
        self
    }
}

impl AstFile {
    pub fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::new(
            "source_file",
            json_span(sources, self.span),
            self.items
                .iter()
                .map(|item| item.to_json(sources))
                .collect(),
        )
    }
}

impl Item {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            Item::Use(item) => JsonAstNode::with_value(
                "use_item",
                item.path.value.clone(),
                json_span(sources, item.span),
                vec![item.path.to_json(sources)],
            ),
            Item::Import(item) => JsonAstNode::with_value(
                "import_item",
                item.path.value.clone(),
                json_span(sources, item.span),
                vec![item.path.to_json(sources), item.alias.to_json(sources)],
            ),
            Item::FromImport(item) => {
                let mut children = vec![item.path.to_json(sources)];
                children.extend(item.names.iter().map(|name| name.to_json(sources)));
                JsonAstNode::with_value(
                    match item.visibility {
                        Visibility::Public => "pub_from_import_item",
                        Visibility::Private | Visibility::Nocter => "from_import_item",
                    },
                    item.path.value.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Program(item) => JsonAstNode::new(
                "program_decl",
                json_span(sources, item.span),
                vec![
                    item.return_type.to_json(sources),
                    item.body.to_json(sources),
                ],
            ),
            Item::Function(item) => JsonAstNode::with_value(
                "function_decl",
                item.name.clone(),
                json_span(sources, item.span),
                vec![
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                    item.parameters.to_json(sources),
                    item.return_type.to_json(sources),
                    item.body.to_json(sources),
                ],
            ),
            Item::Primitive(item) => JsonAstNode::with_value(
                "primitive_decl",
                item.name.clone(),
                json_span(sources, item.span),
                vec![
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                    item.parameters.to_json(sources),
                    item.return_type.to_json(sources),
                ],
            ),
            Item::TypeAlias(item) => JsonAstNode::with_value(
                "type_alias_decl",
                item.name.clone(),
                json_span(sources, item.span),
                vec![
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                    item.target.to_json(sources),
                ],
            ),
            Item::Struct(item) => {
                let mut children = vec![visibility_json(item.visibility)];
                if item.is_copy {
                    children.push(JsonAstNode::new("copy_modifier", None, Vec::new()));
                }
                children.push(item.generics.to_json(sources));
                children.extend(item.fields.iter().map(|field| field.to_json(sources)));
                JsonAstNode::with_value(
                    "struct_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Enum(item) => {
                let mut children = vec![
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                ];
                children.extend(item.variants.iter().map(|variant| variant.to_json(sources)));
                JsonAstNode::with_value(
                    "enum_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Trait(item) => {
                let mut children = vec![
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                ];
                children.extend(item.methods.iter().map(|method| method.to_json(sources)));
                JsonAstNode::with_value(
                    "trait_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Impl(item) => {
                let mut children = Vec::new();
                if let Some(trait_ty) = &item.trait_ty {
                    children.push(JsonAstNode::new(
                        "trait_type",
                        json_span(sources, trait_ty.span()),
                        vec![trait_ty.to_json(sources)],
                    ));
                }
                children.push(JsonAstNode::new(
                    "impl_target_type",
                    json_span(sources, item.target_ty.span()),
                    vec![item.target_ty.to_json(sources)],
                ));
                children.extend(item.members.iter().map(|member| member.to_json(sources)));
                JsonAstNode::new("impl_decl", json_span(sources, item.span), children)
            }
        }
    }
}

impl ModulePath {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "module_path",
            self.value.clone(),
            json_span(sources, self.span),
            Vec::new(),
        )
    }
}

impl ImportAlias {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "import_alias",
            self.name.clone(),
            json_span(sources, self.span),
            Vec::new(),
        )
    }
}

impl ImportedName {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let children = self
            .alias
            .as_ref()
            .map(|alias| vec![alias.to_json(sources)])
            .unwrap_or_default();
        JsonAstNode::with_value(
            "imported_name",
            self.name.clone(),
            json_span(sources, self.span),
            children,
        )
    }
}

impl ParameterList {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::new(
            "parameter_list",
            json_span(sources, self.span),
            self.parameters
                .iter()
                .map(|parameter| parameter.to_json(sources))
                .collect(),
        )
    }
}

impl GenericParamList {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::new(
            "generic_param_list",
            self.span.and_then(|span| json_span(sources, span)),
            self.parameters
                .iter()
                .map(|parameter| parameter.to_json(sources))
                .collect(),
        )
    }
}

impl GenericParam {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let children = self
            .bound
            .as_ref()
            .map(|bound| vec![bound.to_json(sources)])
            .unwrap_or_default();
        JsonAstNode::with_value(
            "generic_param",
            self.name.clone(),
            json_span(sources, self.span),
            children,
        )
    }
}

impl ImplMember {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            ImplMember::Function(function) => JsonAstNode::with_value(
                "associated_function_decl",
                function.name.clone(),
                json_span(sources, function.span),
                vec![
                    visibility_json(function.visibility),
                    function.generics.to_json(sources),
                    function.parameters.to_json(sources),
                    function.return_type.to_json(sources),
                    function.body.to_json(sources),
                ],
            ),
            ImplMember::Method(method) => method.to_json(sources),
        }
    }
}

impl MethodDecl {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let mut children = vec![
            visibility_json(self.visibility),
            JsonAstNode::new(
                "method_receiver",
                json_span(sources, self.receiver.span),
                vec![self.receiver.to_json(sources)],
            ),
            self.parameters.to_json(sources),
            self.return_type.to_json(sources),
        ];
        if let Some(body) = &self.body {
            children.push(body.to_json(sources));
        }

        JsonAstNode::with_value(
            if self.body.is_some() {
                "method_decl"
            } else {
                "method_signature"
            },
            self.name.clone(),
            json_span(sources, self.span),
            children,
        )
    }
}

impl Parameter {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "parameter",
            self.name.clone(),
            json_span(sources, self.span),
            vec![self.ty.to_json(sources)],
        )
    }
}

impl StructField {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "struct_field",
            self.name.clone(),
            json_span(sources, self.span),
            vec![visibility_json(self.visibility), self.ty.to_json(sources)],
        )
    }
}

impl StructLiteralField {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "struct_literal_field",
            self.name.clone(),
            json_span(sources, self.span),
            vec![self.value.to_json(sources)],
        )
    }
}

impl EnumVariant {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "enum_variant",
            self.name.clone(),
            json_span(sources, self.span),
            self.payload
                .iter()
                .map(|parameter| parameter.to_json(sources))
                .collect(),
        )
    }
}

impl TypeExpr {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            TypeExpr::Reference(ty) => JsonAstNode::with_value(
                "type_reference",
                ty.name.clone(),
                json_span(sources, ty.span),
                Vec::new(),
            ),
            TypeExpr::Generic(ty) => JsonAstNode::with_value(
                "generic_type",
                ty.name.clone(),
                json_span(sources, ty.span),
                ty.arguments
                    .iter()
                    .map(|argument| argument.to_json(sources))
                    .collect(),
            ),
            TypeExpr::Pointer(ty) => JsonAstNode::new(
                "pointer_type",
                json_span(sources, ty.span),
                vec![ty.inner.to_json(sources)],
            ),
            TypeExpr::Borrow(ty) => JsonAstNode::new(
                if ty.is_readwrite {
                    "readwrite_borrow_type"
                } else {
                    "readonly_borrow_type"
                },
                json_span(sources, ty.span),
                vec![ty.inner.to_json(sources)],
            ),
            TypeExpr::View(ty) => JsonAstNode::new(
                if ty.is_readwrite {
                    "readwrite_view_type"
                } else {
                    "readonly_view_type"
                },
                json_span(sources, ty.span),
                vec![ty.element.to_json(sources)],
            ),
            TypeExpr::Array(ty) => JsonAstNode::with_value(
                "array_type",
                ty.length.value.clone(),
                json_span(sources, ty.span),
                vec![ty.element.to_json(sources)],
            ),
            TypeExpr::Optional(ty) => JsonAstNode::new(
                "optional_type",
                json_span(sources, ty.span),
                vec![ty.inner.to_json(sources)],
            ),
            TypeExpr::Fallible(ty) => JsonAstNode::new(
                "fallible_type",
                json_span(sources, ty.span),
                vec![ty.success.to_json(sources)],
            ),
        }
    }
}

fn visibility_json(visibility: Visibility) -> JsonAstNode {
    JsonAstNode::with_value(
        "visibility",
        match visibility {
            Visibility::Private => "private",
            Visibility::Public => "pub",
            Visibility::Nocter => "pub(nocter)",
        },
        None,
        Vec::new(),
    )
}

impl Block {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        self.to_json_with_kind(sources, "block")
    }

    fn to_json_with_kind(&self, sources: &SourceMap, kind: &str) -> JsonAstNode {
        JsonAstNode::new(
            kind,
            json_span(sources, self.span),
            self.statements
                .iter()
                .map(|statement| statement.to_json(sources))
                .collect(),
        )
    }
}

impl Stmt {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            Stmt::Return(statement) => JsonAstNode::new(
                "return_statement",
                json_span(sources, statement.span),
                statement
                    .expression
                    .iter()
                    .map(|expression| expression.to_json(sources))
                    .collect(),
            ),
            Stmt::Fail(statement) => JsonAstNode::new(
                "fail_statement",
                json_span(sources, statement.span),
                vec![statement.expression.to_json(sources)],
            ),
            Stmt::Binding(statement) => {
                let mut children = Vec::new();
                if let Some(ty) = &statement.ty {
                    children.push(ty.to_json(sources));
                }
                children.push(statement.initializer.to_json(sources));
                if let Some(else_block) = &statement.else_block {
                    children.push(else_block.to_json_with_kind(sources, "else_block"));
                }
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
                    pattern_children.push(JsonAstNode::with_value(
                        "if_is_payload_binding",
                        payload.name.clone(),
                        json_span(sources, payload.span),
                        Vec::new(),
                    ));
                }

                let mut children = vec![
                    statement.expression.to_json(sources),
                    JsonAstNode::with_value(
                        "if_is_pattern",
                        format!("{}.{}", statement.enum_name, statement.variant_name),
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
            Stmt::IfLet(statement) => {
                let mut children = vec![
                    statement.initializer.to_json(sources),
                    JsonAstNode::with_value(
                        "if_binding",
                        statement.name.clone(),
                        json_span(sources, statement.name_span),
                        Vec::new(),
                    ),
                    statement
                        .then_block
                        .to_json_with_kind(sources, "then_block"),
                ];
                if let Some(else_block) = &statement.else_block {
                    children.push(else_block.to_json_with_kind(sources, "else_block"));
                }
                JsonAstNode::with_value(
                    match statement.kind {
                        BindingKind::Let => "if_let_statement",
                        BindingKind::Var => "if_var_statement",
                    },
                    statement.name.clone(),
                    json_span(sources, statement.span),
                    children,
                )
            }
            Stmt::Switch(statement) => {
                let mut children = vec![statement.expression.to_json(sources)];
                children.extend(statement.arms.iter().map(|arm| arm.to_json(sources)));
                if let Some(else_arm) = &statement.else_arm {
                    children.push(else_arm.to_json(sources));
                }
                JsonAstNode::new(
                    "switch_statement",
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
            Stmt::WhileLet(statement) => JsonAstNode::with_value(
                match statement.kind {
                    BindingKind::Let => "while_let_statement",
                    BindingKind::Var => "while_var_statement",
                },
                statement.name.clone(),
                json_span(sources, statement.span),
                vec![
                    statement.initializer.to_json(sources),
                    JsonAstNode::with_value(
                        "while_binding",
                        statement.name.clone(),
                        json_span(sources, statement.name_span),
                        Vec::new(),
                    ),
                    statement.body.to_json_with_kind(sources, "body"),
                ],
            ),
            Stmt::Loop(statement) => JsonAstNode::new(
                "loop_statement",
                json_span(sources, statement.span),
                vec![statement.body.to_json_with_kind(sources, "body")],
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
            Stmt::Expression(statement) => JsonAstNode::new(
                "expression_statement",
                json_span(sources, statement.span),
                vec![statement.expression.to_json(sources)],
            ),
        }
    }
}

impl SwitchArm {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let mut children = vec![
            JsonAstNode::with_value(
                "switch_pattern",
                format!("{}.{}", self.enum_name, self.variant_name),
                json_span(sources, self.span),
                Vec::new(),
            ),
            self.body.to_json_with_kind(sources, "body"),
        ];
        if let Some(payload) = &self.payload {
            children.insert(
                1,
                JsonAstNode::with_value(
                    "switch_payload_binding",
                    payload.name.clone(),
                    json_span(sources, payload.span),
                    Vec::new(),
                ),
            );
        }

        JsonAstNode::with_value(
            "switch_arm",
            format!("{}.{}", self.enum_name, self.variant_name),
            json_span(sources, self.span),
            children,
        )
    }
}

impl SwitchElseArm {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::new(
            "switch_else_arm",
            json_span(sources, self.span),
            vec![self.body.to_json_with_kind(sources, "body")],
        )
    }
}

impl Expr {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
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
            Expr::StringLiteral(expression) => JsonAstNode::with_value(
                "string_literal",
                expression.value.clone(),
                json_span(sources, expression.span),
                Vec::new(),
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
            Expr::OptionalDefault(expression) => JsonAstNode::new(
                "optional_default_expression",
                json_span(sources, expression.span),
                vec![
                    expression.value.to_json(sources),
                    expression.default.to_json(sources),
                ],
            )
            .with_operator_span(json_span(sources, expression.operator_span)),
        }
    }
}

fn json_span(sources: &SourceMap, span: ByteSpan) -> Option<JsonSpan> {
    sources.span_to_json(span).ok()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AstEnvelope {
    pub schema: &'static str,
    pub version: u32,
    pub ok: bool,
    pub command: &'static str,
    pub file: String,
    pub absolute_path: Option<String>,
    pub ast: Option<JsonAstNode>,
    pub diagnostics: Vec<Diagnostic>,
}

impl AstEnvelope {
    pub fn new(
        file: impl Into<String>,
        absolute_path: Option<String>,
        ast: Option<JsonAstNode>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        let ok = diagnostics.is_empty();

        Self {
            schema: "nocter.ast",
            version: 1,
            ok,
            command: "ast",
            file: file.into(),
            absolute_path,
            ast,
            diagnostics,
        }
    }
}
