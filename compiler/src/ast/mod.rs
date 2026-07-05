//! Source-level abstract syntax tree definitions.

use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, JsonSpan, SourceMap};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonAstNode {
    pub kind: String,
    pub span: Option<JsonSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub items: Vec<JsonAstNode>,
}

impl JsonAstNode {
    pub fn new(kind: impl Into<String>, span: Option<JsonSpan>, items: Vec<JsonAstNode>) -> Self {
        Self {
            kind: kind.into(),
            span,
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
            value: Some(value.into()),
            items,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstFile {
    pub span: ByteSpan,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Use(UseItem),
    FromImport(FromImportItem),
    Program(ProgramDecl),
    Function(FunctionDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseItem {
    pub span: ByteSpan,
    pub path: ModulePath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FromImportItem {
    pub span: ByteSpan,
    pub path: ModulePath,
    pub names: Vec<ImportedName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulePath {
    pub span: ByteSpan,
    pub value: String,
    pub segments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedName {
    pub span: ByteSpan,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramDecl {
    pub span: ByteSpan,
    pub return_type: TypeExpr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDecl {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub parameters: ParameterList,
    pub return_type: TypeExpr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterList {
    pub span: ByteSpan,
    pub parameters: Vec<Parameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpr {
    Reference(TypeReference),
    Optional(OptionalType),
    Fallible(FallibleType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeReference {
    pub span: ByteSpan,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalType {
    pub span: ByteSpan,
    pub inner: Box<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallibleType {
    pub span: ByteSpan,
    pub success: Box<TypeExpr>,
    pub error: Box<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub span: ByteSpan,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Return(ReturnStmt),
    Binding(BindingStmt),
    Try(TryStmt),
    TryCatch(TryCatchStmt),
    Expression(ExpressionStmt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnStmt {
    pub span: ByteSpan,
    pub expression: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    Let,
    Var,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingStmt {
    pub span: ByteSpan,
    pub kind: BindingKind,
    pub name: String,
    pub name_span: ByteSpan,
    pub ty: Option<TypeExpr>,
    pub initializer: Expr,
    pub else_block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryStmt {
    pub span: ByteSpan,
    pub expression: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryCatchStmt {
    pub span: ByteSpan,
    pub expression: Expr,
    pub error_name: String,
    pub error_span: ByteSpan,
    pub catch_block: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionStmt {
    pub span: ByteSpan,
    pub expression: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Identifier(IdentifierExpr),
    IntegerLiteral(LiteralExpr),
    StringLiteral(LiteralExpr),
    NoneLiteral(LiteralExpr),
    Try(TryExpr),
    TryCatch(TryCatchExpr),
    Call(CallExpr),
    Member(MemberExpr),
    Group(GroupExpr),
    OptionalDefault(OptionalDefaultExpr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierExpr {
    pub span: ByteSpan,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralExpr {
    pub span: ByteSpan,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryExpr {
    pub span: ByteSpan,
    pub expression: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryCatchExpr {
    pub span: ByteSpan,
    pub expression: Box<Expr>,
    pub error_name: String,
    pub error_span: ByteSpan,
    pub catch_block: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallExpr {
    pub span: ByteSpan,
    pub callee: Box<Expr>,
    pub arguments_span: ByteSpan,
    pub arguments: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberExpr {
    pub span: ByteSpan,
    pub object: Box<Expr>,
    pub member: String,
    pub member_span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupExpr {
    pub span: ByteSpan,
    pub expression: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalDefaultExpr {
    pub span: ByteSpan,
    pub value: Box<Expr>,
    pub default: Box<Expr>,
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
    pub fn span(&self) -> ByteSpan {
        match self {
            Item::Use(item) => item.span,
            Item::FromImport(item) => item.span,
            Item::Program(item) => item.span,
            Item::Function(item) => item.span,
        }
    }

    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            Item::Use(item) => JsonAstNode::with_value(
                "use_item",
                item.path.value.clone(),
                json_span(sources, item.span),
                vec![item.path.to_json(sources)],
            ),
            Item::FromImport(item) => {
                let mut children = vec![item.path.to_json(sources)];
                children.extend(item.names.iter().map(|name| name.to_json(sources)));
                JsonAstNode::with_value(
                    "from_import_item",
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
                    item.parameters.to_json(sources),
                    item.return_type.to_json(sources),
                    item.body.to_json(sources),
                ],
            ),
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

impl ImportedName {
    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "imported_name",
            self.name.clone(),
            json_span(sources, self.span),
            Vec::new(),
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

impl TypeExpr {
    pub fn span(&self) -> ByteSpan {
        match self {
            TypeExpr::Reference(ty) => ty.span,
            TypeExpr::Optional(ty) => ty.span,
            TypeExpr::Fallible(ty) => ty.span,
        }
    }

    fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            TypeExpr::Reference(ty) => JsonAstNode::with_value(
                "type_reference",
                ty.name.clone(),
                json_span(sources, ty.span),
                Vec::new(),
            ),
            TypeExpr::Optional(ty) => JsonAstNode::new(
                "optional_type",
                json_span(sources, ty.span),
                vec![ty.inner.to_json(sources)],
            ),
            TypeExpr::Fallible(ty) => JsonAstNode::new(
                "fallible_type",
                json_span(sources, ty.span),
                vec![ty.success.to_json(sources), ty.error.to_json(sources)],
            ),
        }
    }
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
    pub fn span(&self) -> ByteSpan {
        match self {
            Stmt::Return(statement) => statement.span,
            Stmt::Binding(statement) => statement.span,
            Stmt::Try(statement) => statement.span,
            Stmt::TryCatch(statement) => statement.span,
            Stmt::Expression(statement) => statement.span,
        }
    }

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
            Stmt::Try(statement) => JsonAstNode::new(
                "try_statement",
                json_span(sources, statement.span),
                vec![statement.expression.to_json(sources)],
            ),
            Stmt::TryCatch(statement) => JsonAstNode::new(
                "try_catch_statement",
                json_span(sources, statement.span),
                vec![
                    statement.expression.to_json(sources),
                    JsonAstNode::with_value(
                        "catch_binding",
                        statement.error_name.clone(),
                        json_span(sources, statement.error_span),
                        Vec::new(),
                    ),
                    statement.catch_block.to_json(sources),
                ],
            ),
            Stmt::Expression(statement) => JsonAstNode::new(
                "expression_statement",
                json_span(sources, statement.span),
                vec![statement.expression.to_json(sources)],
            ),
        }
    }
}

impl Expr {
    pub fn span(&self) -> ByteSpan {
        match self {
            Expr::Identifier(expression) => expression.span,
            Expr::IntegerLiteral(expression) => expression.span,
            Expr::StringLiteral(expression) => expression.span,
            Expr::NoneLiteral(expression) => expression.span,
            Expr::Try(expression) => expression.span,
            Expr::TryCatch(expression) => expression.span,
            Expr::Call(expression) => expression.span,
            Expr::Member(expression) => expression.span,
            Expr::Group(expression) => expression.span,
            Expr::OptionalDefault(expression) => expression.span,
        }
    }

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
            Expr::NoneLiteral(expression) => JsonAstNode::with_value(
                "none_literal",
                expression.value.clone(),
                json_span(sources, expression.span),
                Vec::new(),
            ),
            Expr::Try(expression) => JsonAstNode::new(
                "try_expression",
                json_span(sources, expression.span),
                vec![expression.expression.to_json(sources)],
            ),
            Expr::TryCatch(expression) => JsonAstNode::new(
                "try_catch_expression",
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
            ),
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
            ),
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
