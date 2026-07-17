//! Source-level abstract syntax tree definitions.

mod documentation;
mod json;

pub use json::{AstEnvelope, JsonAstNode};

use crate::source::ByteSpan;

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
pub struct FunctionDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub target: Option<TargetDirective>,
    pub owner: Option<FunctionOwner>,
    pub name: String,
    pub name_span: ByteSpan,
    pub member_name: String,
    pub member_name_span: ByteSpan,
    pub generics: GenericParamList,
    pub parameters: ParameterList,
    pub return_type: TypeExpr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionOwner {
    pub name: String,
    pub name_span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub target: Option<TargetDirective>,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub parameters: ParameterList,
    pub return_type: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDirective {
    pub span: ByteSpan,
    pub target_span: ByteSpan,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAliasDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub target_directive: Option<TargetDirective>,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub target: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub target: Option<TargetDirective>,
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
    pub target: Option<TargetDirective>,
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
    Method(MethodDecl),
    Drop(DropDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropDecl {
    pub span: ByteSpan,
    pub binding: Parameter,
    pub body: Block,
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
    Binding(BindingStmt),
    Assignment(AssignmentStmt),
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
    Drop(DropStmt),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentOperator {
    Assign,
    AddAssign,
    SubtractAssign,
    MultiplyAssign,
    DivideAssign,
    RemainderAssign,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentStmt {
    pub span: ByteSpan,
    pub target: Expr,
    pub operator: AssignmentOperator,
    pub operator_span: ByteSpan,
    pub value: Expr,
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
pub struct DropStmt {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
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
    InterpolatedString(InterpolatedStringExpr),
    BoolLiteral(LiteralExpr),
    NoneLiteral(LiteralExpr),
    ArrayLiteral(ArrayLiteralExpr),
    StructLiteral(StructLiteralExpr),
    Propagate(PropagationExpr),
    Force(ForceExpr),
    Catch(CatchExpr),
    Borrow(BorrowExpr),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    TypeConversion(TypeConversionExpr),
    Call(CallExpr),
    Member(MemberExpr),
    Index(IndexExpr),
    Group(GroupExpr),
    OptionalDefault(OptionalDefaultExpr),
    PatternConditional(PatternConditionalExpr),
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
pub struct InterpolatedStringExpr {
    pub span: ByteSpan,
    pub value: String,
    pub parts: Vec<InterpolatedStringPart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterpolatedStringPart {
    Text(InterpolatedStringText),
    Expression(InterpolatedStringExpression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpolatedStringText {
    pub span: ByteSpan,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpolatedStringExpression {
    pub span: ByteSpan,
    pub expression_span: ByteSpan,
    pub expression: Box<Expr>,
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
pub struct PropagationExpr {
    pub span: ByteSpan,
    pub operator_span: ByteSpan,
    pub expression: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForceExpr {
    pub span: ByteSpan,
    pub operator_span: ByteSpan,
    pub expression: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatchExpr {
    pub span: ByteSpan,
    pub catch_span: ByteSpan,
    pub expression: Box<Expr>,
    pub error_name: String,
    pub error_span: ByteSpan,
    pub catch_block: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowExpr {
    pub span: ByteSpan,
    pub operator_span: ByteSpan,
    pub is_readwrite: bool,
    pub expression: Box<Expr>,
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
    Move,
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
    pub operator_span: ByteSpan,
    pub value: Box<Expr>,
    pub default: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternConditionalExpr {
    pub span: ByteSpan,
    pub question_span: ByteSpan,
    pub target: Box<Expr>,
    pub arms: Vec<PatternConditionalArm>,
    pub fallback_colon_span: ByteSpan,
    pub fallback: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternConditionalArm {
    pub span: ByteSpan,
    pub enum_name: String,
    pub enum_name_span: ByteSpan,
    pub variant_name: String,
    pub variant_name_span: ByteSpan,
    pub payload: Option<SwitchPayloadBinding>,
    pub colon_span: ByteSpan,
    pub expression: Expr,
}

impl Item {
    pub fn span(&self) -> ByteSpan {
        match self {
            Item::Use(item) => item.span,
            Item::Import(item) => item.span,
            Item::FromImport(item) => item.span,
            Item::Function(item) => item.span,
            Item::Primitive(item) => item.span,
            Item::TypeAlias(item) => item.span,
            Item::Struct(item) => item.span,
            Item::Enum(item) => item.span,
            Item::Trait(item) => item.span,
            Item::Impl(item) => item.span,
        }
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
}

impl GenericParamList {
    pub fn empty() -> Self {
        Self {
            span: None,
            parameters: Vec::new(),
        }
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
}

impl Stmt {
    pub fn span(&self) -> ByteSpan {
        match self {
            Stmt::Return(statement) => statement.span,
            Stmt::Binding(statement) => statement.span,
            Stmt::Assignment(statement) => statement.span,
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
            Stmt::Drop(statement) => statement.span,
            Stmt::Expression(statement) => statement.span,
        }
    }
}

impl Expr {
    pub fn span(&self) -> ByteSpan {
        match self {
            Expr::Identifier(expression) => expression.span,
            Expr::IntegerLiteral(expression) => expression.span,
            Expr::StringLiteral(expression) => expression.span,
            Expr::InterpolatedString(expression) => expression.span,
            Expr::BoolLiteral(expression) => expression.span,
            Expr::NoneLiteral(expression) => expression.span,
            Expr::ArrayLiteral(expression) => expression.span,
            Expr::StructLiteral(expression) => expression.span,
            Expr::Propagate(expression) => expression.span,
            Expr::Force(expression) => expression.span,
            Expr::Catch(expression) => expression.span,
            Expr::Borrow(expression) => expression.span,
            Expr::Unary(expression) => expression.span,
            Expr::Binary(expression) => expression.span,
            Expr::TypeConversion(expression) => expression.span,
            Expr::Call(expression) => expression.span,
            Expr::Member(expression) => expression.span,
            Expr::Index(expression) => expression.span,
            Expr::Group(expression) => expression.span,
            Expr::OptionalDefault(expression) => expression.span,
            Expr::PatternConditional(expression) => expression.span,
        }
    }
}

impl UnaryOperator {
    pub fn spelling(self) -> &'static str {
        match self {
            UnaryOperator::LogicalNot => "!",
            UnaryOperator::Negate => "-",
            UnaryOperator::Move => "move",
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
