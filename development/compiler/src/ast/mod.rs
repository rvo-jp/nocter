//! Source-level abstract syntax tree definitions.

mod callables;
mod closures;
mod coercions;
mod collection_for;
mod constructs;
mod declaration_patterns;
mod documentation;
mod generic_requirements;
mod json;
mod literals;
mod packages;
mod provenance;
mod receivers;
mod tests;
mod type_notation;
mod types;
mod visit;

pub use callables::*;
pub use closures::*;
pub use coercions::*;
pub use collection_for::*;
pub use constructs::*;
pub(crate) use declaration_patterns::{
    declaration_patterns_overlap, declaration_patterns_overlap_with_names,
};
pub use generic_requirements::*;
pub use json::{AstEnvelope, JsonAstNode};
pub use literals::*;
pub use packages::*;
pub use provenance::*;
pub use receivers::*;
pub use tests::*;
pub(crate) use type_notation::canonical_type_expr;
pub(crate) use types::substitute_type_expr_parameters;
pub(crate) use visit::{
    closure_expression_by_span, visit_block_expressions_without_nested_closures, visit_expression,
    visit_file_expressions,
};

use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstFile {
    pub span: ByteSpan,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Import(ImportItem),
    FromImport(FromImportItem),
    Function(FunctionDecl),
    Test(TestDecl),
    Primitive(PrimitiveDecl),
    TypeAlias(TypeAliasDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Interface(InterfaceDecl),
    Instance(InstanceDecl),
    Conformance(ConformanceDecl),
    Construct(ConstructDecl),
    Coerce(CoerceDecl),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Private,
    /// The declaring module subtree (`0`) or one of its ancestor subtrees.
    ModuleTree(u16),
    /// Every module in the declaring package.
    Package,
    Public,
}

impl Visibility {
    pub fn source_notation(self) -> String {
        match self {
            Self::Private => String::new(),
            Self::ModuleTree(0) => "pub(./)".to_string(),
            Self::ModuleTree(parents) => format!("pub({})", "../".repeat(parents.into())),
            Self::Package => "pub(/)".to_string(),
            Self::Public => "pub".to_string(),
        }
    }

    pub fn is_private(self) -> bool {
        self == Self::Private
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportItem {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub path: ModulePath,
    pub alias: ImportAlias,
    pub alias_is_default: bool,
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
    pub segment_spans: Vec<ByteSpan>,
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
    pub keyword_span: ByteSpan,
    pub owner: Option<FunctionOwner>,
    pub name: String,
    pub name_span: ByteSpan,
    pub member_name: String,
    pub member_name_span: ByteSpan,
    pub generics: GenericParamList,
    pub parameters: ParameterList,
    pub return_type: TypeExpr,
    pub result_provenance: Option<ResultProvenanceClause>,
    pub requirements: Option<WhereClause>,
    pub body: Option<Block>,
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
    pub keyword_span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub parameters: ParameterList,
    pub return_type: TypeExpr,
    pub result_provenance: Option<ResultProvenanceClause>,
    pub requirements: Option<WhereClause>,
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
    pub requirements: Option<WhereClause>,
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
    pub requirements: Option<WhereClause>,
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
    pub requirements: Option<WhereClause>,
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
pub struct InterfaceDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub target: Option<TargetDirective>,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub requirements: Option<WhereClause>,
    pub associated_types: Vec<AssociatedTypeDecl>,
    pub methods: Vec<MethodDecl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociatedTypeDecl {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub bounds: Vec<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceDecl {
    pub span: ByteSpan,
    pub generics: GenericParamList,
    pub target_ty: TypeExpr,
    pub requirements: Option<WhereClause>,
    pub members: Vec<InstanceMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceMember {
    Method(MethodDecl),
    Drop(DropDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceDecl {
    pub span: ByteSpan,
    pub generics: GenericParamList,
    pub interface_ty: TypeExpr,
    pub target_ty: TypeExpr,
    pub requirements: Option<WhereClause>,
    pub members: Vec<ConformanceMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConformanceMember {
    AssociatedType(AssociatedTypeBinding),
    Method(MethodDecl),
}

pub trait MethodOwnerDecl {
    fn span(&self) -> ByteSpan;
    fn generics(&self) -> &GenericParamList;
    fn target_ty(&self) -> &TypeExpr;
    fn requirements(&self) -> Option<&WhereClause>;
    fn methods(&self) -> Box<dyn Iterator<Item = &MethodDecl> + '_>;
    fn drop_decl(&self) -> Option<&DropDecl>;
}

impl MethodOwnerDecl for InstanceDecl {
    fn span(&self) -> ByteSpan {
        self.span
    }

    fn generics(&self) -> &GenericParamList {
        &self.generics
    }

    fn target_ty(&self) -> &TypeExpr {
        &self.target_ty
    }

    fn requirements(&self) -> Option<&WhereClause> {
        self.requirements.as_ref()
    }

    fn methods(&self) -> Box<dyn Iterator<Item = &MethodDecl> + '_> {
        Box::new(self.members.iter().filter_map(|member| match member {
            InstanceMember::Method(method) => Some(method),
            InstanceMember::Drop(_) => None,
        }))
    }

    fn drop_decl(&self) -> Option<&DropDecl> {
        self.members.iter().find_map(|member| match member {
            InstanceMember::Method(_) => None,
            InstanceMember::Drop(drop_) => Some(drop_),
        })
    }
}

impl MethodOwnerDecl for ConformanceDecl {
    fn span(&self) -> ByteSpan {
        self.span
    }

    fn generics(&self) -> &GenericParamList {
        &self.generics
    }

    fn target_ty(&self) -> &TypeExpr {
        &self.target_ty
    }

    fn requirements(&self) -> Option<&WhereClause> {
        self.requirements.as_ref()
    }

    fn methods(&self) -> Box<dyn Iterator<Item = &MethodDecl> + '_> {
        Box::new(self.members.iter().filter_map(|member| match member {
            ConformanceMember::AssociatedType(_) => None,
            ConformanceMember::Method(method) => Some(method),
        }))
    }

    fn drop_decl(&self) -> Option<&DropDecl> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociatedTypeBinding {
    pub span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub value: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropDecl {
    pub span: ByteSpan,
    pub name_span: ByteSpan,
    pub binding: Parameter,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodDecl {
    pub span: ByteSpan,
    pub visibility: Visibility,
    pub keyword_span: ByteSpan,
    pub receiver: MethodReceiver,
    pub name: String,
    pub name_span: ByteSpan,
    pub generics: GenericParamList,
    pub parameters: ParameterList,
    pub return_type: TypeExpr,
    pub result_provenance: Option<ResultProvenanceClause>,
    pub requirements: Option<WhereClause>,
    pub body: Option<Block>,
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
    Callable(CallableTypeExpr),
    Closure(ClosureTypeExpr),
    Reference(TypeReference),
    Generic(GenericType),
    Projection(ProjectedType),
    Pointer(PointerType),
    Borrow(BorrowType),
    View(ViewType),
    Array(ArrayType),
    Optional(OptionalType),
    Fallible(FallibleType),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedType {
    pub span: ByteSpan,
    pub base: Box<TypeExpr>,
    pub name: String,
    pub name_span: ByteSpan,
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
    pub result: Option<Box<Expr>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Import(ImportItem),
    FromImport(FromImportItem),
    Return(ReturnStmt),
    Binding(BindingStmt),
    Assignment(AssignmentStmt),
    If(IfStmt),
    IfIs(IfIsStmt),
    Switch(SwitchStmt),
    ForRange(ForRangeStmt),
    CollectionFor(CollectionForStmt),
    LiteralPackFor(LiteralPackForStmt),
    While(WhileStmt),
    Loop(LoopStmt),
    Region(RegionStmt),
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
    pub payload: Option<SwitchPayloadPattern>,
    pub then_block: Block,
    pub else_block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchStmt {
    pub span: ByteSpan,
    pub expression: Expr,
    pub arms: Vec<SwitchArm>,
    pub wildcard_arm: Option<SwitchWildcardArm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchArm {
    pub span: ByteSpan,
    pub enum_name: String,
    pub enum_name_span: ByteSpan,
    pub variant_name: String,
    pub variant_name_span: ByteSpan,
    pub payload: Option<SwitchPayloadPattern>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchPayloadPattern {
    Binding(SwitchPayloadBinding),
    Discard(SwitchPayloadDiscard),
}

impl SwitchPayloadPattern {
    pub fn span(&self) -> ByteSpan {
        match self {
            SwitchPayloadPattern::Binding(binding) => binding.span,
            SwitchPayloadPattern::Discard(discard) => discard.span,
        }
    }

    pub fn binding(&self) -> Option<&SwitchPayloadBinding> {
        match self {
            SwitchPayloadPattern::Binding(binding) => Some(binding),
            SwitchPayloadPattern::Discard(_) => None,
        }
    }

    pub fn binding_name(&self) -> Option<&str> {
        self.binding().map(|binding| binding.name.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchPayloadBinding {
    pub span: ByteSpan,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchPayloadDiscard {
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchWildcardArm {
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
pub struct LoopStmt {
    pub span: ByteSpan,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionStmt {
    pub span: ByteSpan,
    pub keyword_span: ByteSpan,
    pub name: String,
    pub name_span: ByteSpan,
    pub using_span: ByteSpan,
    pub allocator: Expr,
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
    Closure(ClosureExpr),
    Identifier(IdentifierExpr),
    IntegerLiteral(LiteralExpr),
    ByteLiteral(LiteralExpr),
    StringLiteral(LiteralExpr),
    InterpolatedString(InterpolatedStringExpr),
    BoolLiteral(LiteralExpr),
    NoneLiteral(LiteralExpr),
    ArrayLiteral(ArrayLiteralExpr),
    TypedSequenceLiteral(TypedSequenceLiteralExpr),
    TypedStringLiteral(TypedStringLiteralExpr),
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
    Otherwise(OtherwiseExpr),
    If(Box<IfStmt>),
    IfIs(Box<IfIsStmt>),
    Match(Box<SwitchStmt>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadEnumPatternTargetShape {
    Identifier,
    Call,
    Member,
    PropagatedCall,
    ForcedCall,
    CaughtCall,
    OtherwiseCall,
    MovedIdentifier,
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
    Spread,
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
pub struct OtherwiseExpr {
    pub span: ByteSpan,
    pub keyword_span: ByteSpan,
    pub value: Box<Expr>,
    pub fallback: Block,
}

impl Item {
    pub fn method_owner(&self) -> Option<&dyn MethodOwnerDecl> {
        match self {
            Item::Instance(instance) => Some(instance),
            Item::Conformance(conformance) => Some(conformance),
            _ => None,
        }
    }

    pub fn span(&self) -> ByteSpan {
        match self {
            Item::Import(item) => item.span,
            Item::FromImport(item) => item.span,
            Item::Function(item) => item.span,
            Item::Test(item) => item.span,
            Item::Primitive(item) => item.span,
            Item::TypeAlias(item) => item.span,
            Item::Struct(item) => item.span,
            Item::Enum(item) => item.span,
            Item::Interface(item) => item.span,
            Item::Instance(item) => item.span,
            Item::Conformance(item) => item.span,
            Item::Construct(item) => item.span,
            Item::Coerce(item) => item.span,
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

impl TypeExpr {
    pub fn span(&self) -> ByteSpan {
        match self {
            TypeExpr::Callable(ty) => ty.span,
            TypeExpr::Closure(ty) => ty.span,
            TypeExpr::Reference(ty) => ty.span,
            TypeExpr::Generic(ty) => ty.span,
            TypeExpr::Projection(ty) => ty.span,
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
            Stmt::Import(statement) => statement.span,
            Stmt::FromImport(statement) => statement.span,
            Stmt::Return(statement) => statement.span,
            Stmt::Binding(statement) => statement.span,
            Stmt::Assignment(statement) => statement.span,
            Stmt::If(statement) => statement.span,
            Stmt::IfIs(statement) => statement.span,
            Stmt::Switch(statement) => statement.span,
            Stmt::ForRange(statement) => statement.span,
            Stmt::CollectionFor(statement) => statement.span,
            Stmt::LiteralPackFor(statement) => statement.span,
            Stmt::While(statement) => statement.span,
            Stmt::Loop(statement) => statement.span,
            Stmt::Region(statement) => statement.span,
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
            Expr::Closure(expression) => expression.span,
            Expr::Identifier(expression) => expression.span,
            Expr::IntegerLiteral(expression) => expression.span,
            Expr::ByteLiteral(expression) => expression.span,
            Expr::StringLiteral(expression) => expression.span,
            Expr::InterpolatedString(expression) => expression.span,
            Expr::BoolLiteral(expression) => expression.span,
            Expr::NoneLiteral(expression) => expression.span,
            Expr::ArrayLiteral(expression) => expression.span,
            Expr::TypedSequenceLiteral(expression) => expression.span,
            Expr::TypedStringLiteral(expression) => expression.span,
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
            Expr::Otherwise(expression) => expression.span,
            Expr::If(expression) => expression.span,
            Expr::IfIs(expression) => expression.span,
            Expr::Match(expression) => expression.span,
        }
    }

    pub fn without_groups(&self) -> &Expr {
        match self {
            Expr::Group(group) => group.expression.without_groups(),
            _ => self,
        }
    }

    pub fn is_direct_slice_index_assignment_object(&self) -> bool {
        match self.without_groups() {
            Expr::Identifier(_) | Expr::Call(_) => true,
            Expr::Propagate(propagation) => {
                matches!(propagation.expression.without_groups(), Expr::Call(_))
            }
            Expr::Force(force) => matches!(force.expression.without_groups(), Expr::Call(_)),
            Expr::Catch(catch) => matches!(catch.expression.without_groups(), Expr::Call(_)),
            _ => false,
        }
    }

    pub fn payload_enum_pattern_target_shape(&self) -> Option<PayloadEnumPatternTargetShape> {
        match self.without_groups() {
            Expr::Identifier(_) => Some(PayloadEnumPatternTargetShape::Identifier),
            Expr::Call(_) => Some(PayloadEnumPatternTargetShape::Call),
            Expr::Member(_) => Some(PayloadEnumPatternTargetShape::Member),
            Expr::Propagate(propagation)
                if matches!(propagation.expression.without_groups(), Expr::Call(_)) =>
            {
                Some(PayloadEnumPatternTargetShape::PropagatedCall)
            }
            Expr::Force(force) if matches!(force.expression.without_groups(), Expr::Call(_)) => {
                Some(PayloadEnumPatternTargetShape::ForcedCall)
            }
            Expr::Catch(catch) if matches!(catch.expression.without_groups(), Expr::Call(_)) => {
                Some(PayloadEnumPatternTargetShape::CaughtCall)
            }
            Expr::Otherwise(otherwise)
                if matches!(otherwise.value.without_groups(), Expr::Call(_)) =>
            {
                Some(PayloadEnumPatternTargetShape::OtherwiseCall)
            }
            Expr::Unary(unary)
                if unary.operator == UnaryOperator::Move
                    && matches!(unary.operand.without_groups(), Expr::Identifier(_)) =>
            {
                Some(PayloadEnumPatternTargetShape::MovedIdentifier)
            }
            _ => None,
        }
    }
}

impl UnaryOperator {
    pub fn spelling(self) -> &'static str {
        match self {
            UnaryOperator::LogicalNot => "!",
            UnaryOperator::Negate => "-",
            UnaryOperator::Move => "move",
            UnaryOperator::Spread => "...",
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
