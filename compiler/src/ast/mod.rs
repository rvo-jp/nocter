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
    Import(ImportItem),
    FromImport(FromImportItem),
    Program(ProgramDecl),
    Function(FunctionDecl),
    Primitive(PrimitiveDecl),
    TypeAlias(TypeAliasDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplDecl),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Public,
    Nocter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseItem {
    pub span: ByteSpan,
    pub path: ModulePath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportItem {
    pub span: ByteSpan,
    pub path: ModulePath,
    pub alias: ImportAlias,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FromImportItem {
    pub span: ByteSpan,
    pub visibility: Visibility,
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
pub struct ImportAlias {
    pub span: ByteSpan,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedName {
    pub span: ByteSpan,
    pub name_span: ByteSpan,
    pub name: String,
    pub alias: Option<ImportAlias>,
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
    pub visibility: Visibility,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub parameters: ParameterList,
    pub return_type: TypeExpr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub parameters: ParameterList,
    pub return_type: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAliasDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub target: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub is_copy: bool,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub name: String,
    pub name_span: ByteSpan,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub payload: Vec<Parameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub methods: Vec<MethodDecl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplDecl {
    pub span: ByteSpan,
    pub trait_ty: Option<TypeExpr>,
    pub target_ty: TypeExpr,
    pub members: Vec<ImplMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImplMember {
    Function(FunctionDecl),
    Method(MethodDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub receiver: Parameter,
    pub name: String,
    pub name_span: ByteSpan,
    pub parameters: ParameterList,
    pub return_type: TypeExpr,
    pub body: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParamList {
    pub span: Option<ByteSpan>,
    pub parameters: Vec<GenericParam>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParam {
    pub span: ByteSpan,
    pub name: String,
    pub bound: Option<TypeExpr>,
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
    Generic(GenericType),
    Pointer(PointerType),
    Borrow(BorrowType),
    View(ViewType),
    Array(ArrayType),
    Optional(OptionalType),
    Fallible(FallibleType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeReference {
    pub span: ByteSpan,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericType {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub arguments: Vec<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointerType {
    pub span: ByteSpan,
    pub inner: Box<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowType {
    pub span: ByteSpan,
    pub is_readwrite: bool,
    pub inner: Box<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewType {
    pub span: ByteSpan,
    pub is_readwrite: bool,
    pub element: Box<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayType {
    pub span: ByteSpan,
    pub element: Box<TypeExpr>,
    pub length: ArrayLength,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayLength {
    pub span: ByteSpan,
    pub value: String,
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
    Fail(FailStmt),
    Binding(BindingStmt),
    Try(TryStmt),
    TryCatch(TryCatchStmt),
    If(IfStmt),
    IfIs(IfIsStmt),
    IfLet(IfLetStmt),
    Switch(SwitchStmt),
    ForRange(ForRangeStmt),
    While(WhileStmt),
    WhileLet(WhileLetStmt),
    Loop(LoopStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
    Expression(ExpressionStmt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnStmt {
    pub span: ByteSpan,
    pub expression: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailStmt {
    pub span: ByteSpan,
    pub expression: Expr,
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
pub struct IfStmt {
    pub span: ByteSpan,
    pub condition: Expr,
    pub then_block: Block,
    pub else_block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfIsStmt {
    pub span: ByteSpan,
    pub expression: Expr,
    pub pattern_span: ByteSpan,
    pub enum_name: String,
    pub enum_name_span: ByteSpan,
    pub variant_name: String,
    pub variant_name_span: ByteSpan,
    pub payload: Option<SwitchPayloadBinding>,
    pub then_block: Block,
    pub else_block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfLetStmt {
    pub span: ByteSpan,
    pub kind: BindingKind,
    pub name: String,
    pub name_span: ByteSpan,
    pub initializer: Expr,
    pub then_block: Block,
    pub else_block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchStmt {
    pub span: ByteSpan,
    pub expression: Expr,
    pub arms: Vec<SwitchArm>,
    pub else_arm: Option<SwitchElseArm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchArm {
    pub span: ByteSpan,
    pub enum_name: String,
    pub enum_name_span: ByteSpan,
    pub variant_name: String,
    pub variant_name_span: ByteSpan,
    pub payload: Option<SwitchPayloadBinding>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchPayloadBinding {
    pub span: ByteSpan,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchElseArm {
    pub span: ByteSpan,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForRangeStmt {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub start: Expr,
    pub range_span: ByteSpan,
    pub end: Expr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileStmt {
    pub span: ByteSpan,
    pub condition: Expr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileLetStmt {
    pub span: ByteSpan,
    pub kind: BindingKind,
    pub name: String,
    pub name_span: ByteSpan,
    pub initializer: Expr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopStmt {
    pub span: ByteSpan,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakStmt {
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinueStmt {
    pub span: ByteSpan,
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
    BoolLiteral(LiteralExpr),
    NoneLiteral(LiteralExpr),
    ArrayLiteral(ArrayLiteralExpr),
    StructLiteral(StructLiteralExpr),
    Try(TryExpr),
    TryCatch(TryCatchExpr),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    TypeConversion(TypeConversionExpr),
    Call(CallExpr),
    Member(MemberExpr),
    Index(IndexExpr),
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
pub struct ArrayLiteralExpr {
    pub span: ByteSpan,
    pub elements_span: ByteSpan,
    pub elements: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteralExpr {
    pub span: ByteSpan,
    pub ty: TypeExpr,
    pub fields_span: ByteSpan,
    pub fields: Vec<StructLiteralField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteralField {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub value: Expr,
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
pub struct UnaryExpr {
    pub span: ByteSpan,
    pub operator: UnaryOperator,
    pub operator_span: ByteSpan,
    pub operand: Box<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    LogicalNot,
    Negate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpr {
    pub span: ByteSpan,
    pub left: Box<Expr>,
    pub operator: BinaryOperator,
    pub operator_span: ByteSpan,
    pub right: Box<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    LogicalAnd,
    LogicalOr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeConversionExpr {
    pub span: ByteSpan,
    pub expression: Box<Expr>,
    pub as_span: ByteSpan,
    pub ty: TypeExpr,
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
pub struct IndexExpr {
    pub span: ByteSpan,
    pub object: Box<Expr>,
    pub index_span: ByteSpan,
    pub index: Box<Expr>,
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
            Item::Import(item) => item.span,
            Item::FromImport(item) => item.span,
            Item::Program(item) => item.span,
            Item::Function(item) => item.span,
            Item::Primitive(item) => item.span,
            Item::TypeAlias(item) => item.span,
            Item::Struct(item) => item.span,
            Item::Enum(item) => item.span,
            Item::Trait(item) => item.span,
            Item::Impl(item) => item.span,
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
    pub fn local_name(&self) -> &str {
        self.alias
            .as_ref()
            .map(|alias| alias.name.as_str())
            .unwrap_or(&self.name)
    }

    pub fn local_span(&self) -> ByteSpan {
        self.alias
            .as_ref()
            .map(|alias| alias.span)
            .unwrap_or(self.name_span)
    }

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
    pub fn empty() -> Self {
        Self {
            span: None,
            parameters: Vec::new(),
        }
    }

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
    pub fn span(&self) -> ByteSpan {
        match self {
            TypeExpr::Reference(ty) => ty.span,
            TypeExpr::Generic(ty) => ty.span,
            TypeExpr::Pointer(ty) => ty.span,
            TypeExpr::Borrow(ty) => ty.span,
            TypeExpr::View(ty) => ty.span,
            TypeExpr::Array(ty) => ty.span,
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
    pub fn span(&self) -> ByteSpan {
        match self {
            Stmt::Return(statement) => statement.span,
            Stmt::Fail(statement) => statement.span,
            Stmt::Binding(statement) => statement.span,
            Stmt::Try(statement) => statement.span,
            Stmt::TryCatch(statement) => statement.span,
            Stmt::If(statement) => statement.span,
            Stmt::IfIs(statement) => statement.span,
            Stmt::IfLet(statement) => statement.span,
            Stmt::Switch(statement) => statement.span,
            Stmt::ForRange(statement) => statement.span,
            Stmt::While(statement) => statement.span,
            Stmt::WhileLet(statement) => statement.span,
            Stmt::Loop(statement) => statement.span,
            Stmt::Break(statement) => statement.span,
            Stmt::Continue(statement) => statement.span,
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
    pub fn span(&self) -> ByteSpan {
        match self {
            Expr::Identifier(expression) => expression.span,
            Expr::IntegerLiteral(expression) => expression.span,
            Expr::StringLiteral(expression) => expression.span,
            Expr::BoolLiteral(expression) => expression.span,
            Expr::NoneLiteral(expression) => expression.span,
            Expr::ArrayLiteral(expression) => expression.span,
            Expr::StructLiteral(expression) => expression.span,
            Expr::Try(expression) => expression.span,
            Expr::TryCatch(expression) => expression.span,
            Expr::Unary(expression) => expression.span,
            Expr::Binary(expression) => expression.span,
            Expr::TypeConversion(expression) => expression.span,
            Expr::Call(expression) => expression.span,
            Expr::Member(expression) => expression.span,
            Expr::Index(expression) => expression.span,
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
            Expr::Try(expression) => JsonAstNode::new(
                "fallible_propagation_expression",
                json_span(sources, expression.span),
                vec![expression.expression.to_json(sources)],
            ),
            Expr::TryCatch(expression) => JsonAstNode::new(
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
            ),
            Expr::Unary(expression) => JsonAstNode::with_value(
                "unary_expression",
                expression.operator.spelling(),
                json_span(sources, expression.span),
                vec![expression.operand.to_json(sources)],
            ),
            Expr::Binary(expression) => JsonAstNode::with_value(
                "binary_expression",
                expression.operator.spelling(),
                json_span(sources, expression.span),
                vec![
                    expression.left.to_json(sources),
                    expression.right.to_json(sources),
                ],
            ),
            Expr::TypeConversion(expression) => JsonAstNode::new(
                "type_conversion_expression",
                json_span(sources, expression.span),
                vec![
                    expression.expression.to_json(sources),
                    expression.ty.to_json(sources),
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
            ),
        }
    }
}

impl UnaryOperator {
    pub fn spelling(self) -> &'static str {
        match self {
            UnaryOperator::LogicalNot => "!",
            UnaryOperator::Negate => "-",
        }
    }
}

impl BinaryOperator {
    pub fn spelling(self) -> &'static str {
        match self {
            BinaryOperator::Add => "+",
            BinaryOperator::Subtract => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::Divide => "/",
            BinaryOperator::Remainder => "%",
            BinaryOperator::ShiftLeft => "<<",
            BinaryOperator::ShiftRight => ">>",
            BinaryOperator::Equal => "==",
            BinaryOperator::NotEqual => "!=",
            BinaryOperator::Less => "<",
            BinaryOperator::LessEqual => "<=",
            BinaryOperator::Greater => ">",
            BinaryOperator::GreaterEqual => ">=",
            BinaryOperator::LogicalAnd => "&&",
            BinaryOperator::LogicalOr => "||",
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
