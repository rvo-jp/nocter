use nocter_source::{ByteOffset, SourceFile, SourceId, Span, TextRange};

use crate::documentation::DocumentationAttachments;
use crate::{ExpectedSyntax, LexedFile, ParseDiagnostic, Token, TokenKind};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NodeKind {
    PackageFile,
    SourceFile,
    PackageDirective,
    DirectiveValue,
    DirectiveRecord,
    DirectiveField,
    StringLiteral,
    IncludeDeclaration,
    IncludePath,
    UseDeclaration,
    Visibility,
    ModulePath,
    ImportSelection,
    SelectedName,
    Item,
    TargetDirective,
    FunctionDeclaration,
    PrimitiveDeclaration,
    TypeAliasDeclaration,
    StructDeclaration,
    StructField,
    EnumDeclaration,
    EnumVariant,
    EnumPayload,
    InterfaceDeclaration,
    AssociatedTypeDeclaration,
    InterfaceBounds,
    InterfaceMethod,
    InterfaceDefaultModifier,
    ConstructDeclaration,
    ConstructionFunction,
    LiteralDeclaration,
    LiteralShape,
    InstanceDeclaration,
    InherentMethod,
    MethodSignature,
    Receiver,
    CoercionDeclaration,
    CoercionProvenance,
    EqualityOperator,
    OrderingOperator,
    IndexOperator,
    ExpansionOperator,
    ConformDeclaration,
    AssociatedTypeBinding,
    ConformMethod,
    DropDeclaration,
    TestDeclaration,
    GenericParameters,
    Parameters,
    Parameter,
    ArgumentPackModifier,
    CallableTail,
    ProvenanceClause,
    Block,
    BlockUseDeclaration,
    ExecutableSequence,
    ExpressionStatement,
    BodyResult,
    BindingStatement,
    BindingTarget,
    TypeAnnotation,
    AssignmentStatement,
    AssignmentTarget,
    ReturnStatement,
    BreakStatement,
    ContinueStatement,
    DropStatement,
    WhileStatement,
    LoopStatement,
    ForStatement,
    ForSource,
    RegionStatement,
    NamedPlace,
    AllocatorPlace,
    Expression,
    RecoveryExpression,
    RecoveryClause,
    LogicalOrExpression,
    LogicalAndExpression,
    EqualityExpression,
    OrderingExpression,
    ShiftExpression,
    AdditiveExpression,
    MultiplicativeExpression,
    ConversionExpression,
    UnaryExpression,
    MoveExpression,
    OutcomeExpression,
    PostfixExpression,
    CallSuffix,
    MemberSuffix,
    IndexSuffix,
    IfExpression,
    IfCondition,
    ElseClause,
    MatchExpression,
    MatchArm,
    EnumPattern,
    EnumPatternPayload,
    PayloadSlot,
    ClosureExpression,
    ClosureHead,
    ClosureCaptures,
    ClosureCapture,
    ClosureParameters,
    ClosureParameter,
    ClosureResult,
    StructLiteral,
    StructInitializer,
    FieldInitializer,
    TypedSequenceLiteral,
    SequenceBody,
    SequenceElement,
    SpreadExpression,
    TypedStringLiteral,
    AllocationOverride,
    ArrayLiteral,
    StringExpression,
    StringPart,
    GenericOwnerMember,
    ReferenceExpression,
    GroupedExpression,
    ScalarLiteral,
    Type,
    BuiltinType,
    SelfType,
    PointerType,
    BorrowType,
    CallableType,
    CallableParameters,
    CallableParameter,
    NamedType,
    TypeArguments,
    SliceType,
    FixedArrayType,
    GroupedType,
    OpaqueResult,
    OpaqueArguments,
    OpaqueArgument,
    WhereClause,
    CapabilityPredicate,
    CopyPredicate,
    TypeEqualityPredicate,
    OperatorPredicate,
    CoercionPredicate,
    ExpansionPredicate,
    Capability,
    DeclarationTypePattern,
    PatternArguments,
    Error,
}

impl NodeKind {
    /// Stable lower-snake-case spelling used by compiler-owned tooling protocols.
    // Keeping this exhaustive protocol vocabulary in one match makes a newly added syntax kind a
    // compile error here instead of silently omitting or deriving a public spelling.
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PackageFile => "package_file",
            Self::SourceFile => "source_file",
            Self::PackageDirective => "package_directive",
            Self::DirectiveValue => "directive_value",
            Self::DirectiveRecord => "directive_record",
            Self::DirectiveField => "directive_field",
            Self::StringLiteral => "string_literal",
            Self::IncludeDeclaration => "include_declaration",
            Self::IncludePath => "include_path",
            Self::UseDeclaration => "use_declaration",
            Self::Visibility => "visibility",
            Self::ModulePath => "module_path",
            Self::ImportSelection => "import_selection",
            Self::SelectedName => "selected_name",
            Self::Item => "item",
            Self::TargetDirective => "target_directive",
            Self::FunctionDeclaration => "function_declaration",
            Self::PrimitiveDeclaration => "primitive_declaration",
            Self::TypeAliasDeclaration => "type_alias_declaration",
            Self::StructDeclaration => "struct_declaration",
            Self::StructField => "struct_field",
            Self::EnumDeclaration => "enum_declaration",
            Self::EnumVariant => "enum_variant",
            Self::EnumPayload => "enum_payload",
            Self::InterfaceDeclaration => "interface_declaration",
            Self::AssociatedTypeDeclaration => "associated_type_declaration",
            Self::InterfaceBounds => "interface_bounds",
            Self::InterfaceMethod => "interface_method",
            Self::InterfaceDefaultModifier => "interface_default_modifier",
            Self::ConstructDeclaration => "construct_declaration",
            Self::ConstructionFunction => "construction_function",
            Self::LiteralDeclaration => "literal_declaration",
            Self::LiteralShape => "literal_shape",
            Self::InstanceDeclaration => "instance_declaration",
            Self::InherentMethod => "inherent_method",
            Self::MethodSignature => "method_signature",
            Self::Receiver => "receiver",
            Self::CoercionDeclaration => "coercion_declaration",
            Self::CoercionProvenance => "coercion_provenance",
            Self::EqualityOperator => "equality_operator",
            Self::OrderingOperator => "ordering_operator",
            Self::IndexOperator => "index_operator",
            Self::ExpansionOperator => "expansion_operator",
            Self::ConformDeclaration => "conform_declaration",
            Self::AssociatedTypeBinding => "associated_type_binding",
            Self::ConformMethod => "conform_method",
            Self::DropDeclaration => "drop_declaration",
            Self::TestDeclaration => "test_declaration",
            Self::GenericParameters => "generic_parameters",
            Self::Parameters => "parameters",
            Self::Parameter => "parameter",
            Self::ArgumentPackModifier => "argument_pack_modifier",
            Self::CallableTail => "callable_tail",
            Self::ProvenanceClause => "provenance_clause",
            Self::Block => "block",
            Self::BlockUseDeclaration => "block_use_declaration",
            Self::ExecutableSequence => "executable_sequence",
            Self::ExpressionStatement => "expression_statement",
            Self::BodyResult => "body_result",
            Self::BindingStatement => "binding_statement",
            Self::BindingTarget => "binding_target",
            Self::TypeAnnotation => "type_annotation",
            Self::AssignmentStatement => "assignment_statement",
            Self::AssignmentTarget => "assignment_target",
            Self::ReturnStatement => "return_statement",
            Self::BreakStatement => "break_statement",
            Self::ContinueStatement => "continue_statement",
            Self::DropStatement => "drop_statement",
            Self::WhileStatement => "while_statement",
            Self::LoopStatement => "loop_statement",
            Self::ForStatement => "for_statement",
            Self::ForSource => "for_source",
            Self::RegionStatement => "region_statement",
            Self::NamedPlace => "named_place",
            Self::AllocatorPlace => "allocator_place",
            Self::Expression => "expression",
            Self::RecoveryExpression => "recovery_expression",
            Self::RecoveryClause => "recovery_clause",
            Self::LogicalOrExpression => "logical_or_expression",
            Self::LogicalAndExpression => "logical_and_expression",
            Self::EqualityExpression => "equality_expression",
            Self::OrderingExpression => "ordering_expression",
            Self::ShiftExpression => "shift_expression",
            Self::AdditiveExpression => "additive_expression",
            Self::MultiplicativeExpression => "multiplicative_expression",
            Self::ConversionExpression => "conversion_expression",
            Self::UnaryExpression => "unary_expression",
            Self::MoveExpression => "move_expression",
            Self::OutcomeExpression => "outcome_expression",
            Self::PostfixExpression => "postfix_expression",
            Self::CallSuffix => "call_suffix",
            Self::MemberSuffix => "member_suffix",
            Self::IndexSuffix => "index_suffix",
            Self::IfExpression => "if_expression",
            Self::IfCondition => "if_condition",
            Self::ElseClause => "else_clause",
            Self::MatchExpression => "match_expression",
            Self::MatchArm => "match_arm",
            Self::EnumPattern => "enum_pattern",
            Self::EnumPatternPayload => "enum_pattern_payload",
            Self::PayloadSlot => "payload_slot",
            Self::ClosureExpression => "closure_expression",
            Self::ClosureHead => "closure_head",
            Self::ClosureCaptures => "closure_captures",
            Self::ClosureCapture => "closure_capture",
            Self::ClosureParameters => "closure_parameters",
            Self::ClosureParameter => "closure_parameter",
            Self::ClosureResult => "closure_result",
            Self::StructLiteral => "struct_literal",
            Self::StructInitializer => "struct_initializer",
            Self::FieldInitializer => "field_initializer",
            Self::TypedSequenceLiteral => "typed_sequence_literal",
            Self::SequenceBody => "sequence_body",
            Self::SequenceElement => "sequence_element",
            Self::SpreadExpression => "spread_expression",
            Self::TypedStringLiteral => "typed_string_literal",
            Self::AllocationOverride => "allocation_override",
            Self::ArrayLiteral => "array_literal",
            Self::StringExpression => "string_expression",
            Self::StringPart => "string_part",
            Self::GenericOwnerMember => "generic_owner_member",
            Self::ReferenceExpression => "reference_expression",
            Self::GroupedExpression => "grouped_expression",
            Self::ScalarLiteral => "scalar_literal",
            Self::Type => "type",
            Self::BuiltinType => "builtin_type",
            Self::SelfType => "self_type",
            Self::PointerType => "pointer_type",
            Self::BorrowType => "borrow_type",
            Self::CallableType => "callable_type",
            Self::CallableParameters => "callable_parameters",
            Self::CallableParameter => "callable_parameter",
            Self::NamedType => "named_type",
            Self::TypeArguments => "type_arguments",
            Self::SliceType => "slice_type",
            Self::FixedArrayType => "fixed_array_type",
            Self::GroupedType => "grouped_type",
            Self::OpaqueResult => "opaque_result",
            Self::OpaqueArguments => "opaque_arguments",
            Self::OpaqueArgument => "opaque_argument",
            Self::WhereClause => "where_clause",
            Self::CapabilityPredicate => "capability_predicate",
            Self::CopyPredicate => "copy_predicate",
            Self::TypeEqualityPredicate => "type_equality_predicate",
            Self::OperatorPredicate => "operator_predicate",
            Self::CoercionPredicate => "coercion_predicate",
            Self::ExpansionPredicate => "expansion_predicate",
            Self::Capability => "capability",
            Self::DeclarationTypePattern => "declaration_type_pattern",
            Self::PatternArguments => "pattern_arguments",
            Self::Error => "error",
        }
    }
}

/// Stable identity of a node inside one immutable syntax tree.
///
/// Nodes and child elements live in flat arenas. This keeps ownership non-recursive even when a
/// valid source contains a very deep chain of prefix types or expressions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId {
    source: SourceId,
    index: usize,
}

impl NodeId {
    const fn new(source: SourceId, index: usize) -> Self {
        Self { source, index }
    }

    #[must_use]
    pub const fn source(self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TokenId(usize);

impl TokenId {
    pub(crate) const fn new(index: usize) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// One syntax-level view of a lexical token.
///
/// Most lexical tokens produce one full-range syntax token. Closed subdivision rules such as a
/// generic `>>` closer produce two non-overlapping syntax tokens with the same lexical identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SyntaxToken {
    source: SourceId,
    lexical: TokenId,
    kind: TokenKind,
    range: TextRange,
}

impl SyntaxToken {
    pub(crate) const fn new(
        source: SourceId,
        lexical: TokenId,
        kind: TokenKind,
        range: TextRange,
    ) -> Self {
        Self {
            source,
            lexical,
            kind,
            range,
        }
    }

    #[must_use]
    pub const fn source(self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn lexical(self) -> TokenId {
        self.lexical
    }

    #[must_use]
    pub const fn kind(self) -> TokenKind {
        self.kind
    }

    #[must_use]
    pub const fn range(self) -> TextRange {
        self.range
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MissingSyntax {
    expected: ExpectedSyntax,
    span: Span,
}

impl MissingSyntax {
    #[must_use]
    pub const fn expected(self) -> ExpectedSyntax {
        self.expected
    }

    #[must_use]
    pub const fn span(self) -> Span {
        self.span
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxElement {
    Node(NodeId),
    Token(SyntaxToken),
    Missing(MissingSyntax),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntaxNode {
    kind: NodeKind,
    range: TextRange,
    first_child: usize,
    child_count: usize,
}

impl SyntaxNode {
    #[must_use]
    pub const fn kind(&self) -> NodeKind {
        self.kind
    }

    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

#[derive(Clone, Debug)]
pub struct SyntaxTree {
    lexed: LexedFile,
    nodes: Vec<SyntaxNode>,
    elements: Vec<SyntaxElement>,
    root: NodeId,
    diagnostics: Vec<ParseDiagnostic>,
    documentation: DocumentationAttachments,
}

impl SyntaxTree {
    pub(crate) fn new(
        source: &SourceFile,
        lexed: LexedFile,
        built: BuiltTree,
        diagnostics: Vec<ParseDiagnostic>,
    ) -> Self {
        let mut tree = Self {
            lexed,
            nodes: built.nodes,
            elements: built.elements,
            root: built.root,
            diagnostics,
            documentation: DocumentationAttachments::empty(),
        };
        tree.documentation = DocumentationAttachments::build(source, &tree);
        tree
    }

    #[must_use]
    pub fn source(&self) -> SourceId {
        self.lexed.source()
    }

    #[must_use]
    pub fn root(&self) -> &SyntaxNode {
        &self.nodes[self.root.index()]
    }

    #[must_use]
    pub const fn root_id(&self) -> NodeId {
        self.root
    }

    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&SyntaxNode> {
        if id.source() != self.source() {
            return None;
        }
        self.nodes.get(id.index())
    }

    /// Iterates every node in its stable arena order.
    ///
    /// The returned identities belong only to this immutable tree. Consumers can use this view to
    /// serialize or index the concrete syntax without reconstructing its arena topology.
    #[must_use]
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = (NodeId, &SyntaxNode)> {
        let source = self.source();
        self.nodes
            .iter()
            .enumerate()
            .map(move |(index, node)| (NodeId::new(source, index), node))
    }

    #[must_use]
    /// Returns the child slice for a node in this tree.
    ///
    /// # Panics
    ///
    /// Panics when `id` belongs to another syntax tree or is not present in this tree.
    pub fn children(&self, id: NodeId) -> &[SyntaxElement] {
        assert_eq!(id.source(), self.source(), "node belongs to another tree");
        let node = &self.nodes[id.index()];
        &self.elements[node.first_child..node.first_child + node.child_count]
    }

    #[must_use]
    pub const fn lexed(&self) -> &LexedFile {
        &self.lexed
    }

    #[must_use]
    pub fn token(&self, id: TokenId) -> Option<&Token> {
        self.lexed.tokens().get(id.index())
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[ParseDiagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.lexed.diagnostics().is_empty() || !self.diagnostics.is_empty()
    }

    /// Returns normalized Markdown documentation attached to the complete source file.
    #[must_use]
    pub fn file_documentation(&self) -> Option<&str> {
        self.documentation.file()
    }

    /// Returns normalized Markdown documentation attached to one documentable syntax node.
    #[must_use]
    pub fn documentation(&self, node: NodeId) -> Option<&str> {
        if node.source() != self.source() {
            return None;
        }
        self.documentation.node(node)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Event {
    Start {
        kind: NodeKind,
        offset: ByteOffset,
        forward_parent: Option<usize>,
    },
    Token(SyntaxToken),
    Missing(MissingSyntax),
    Finish,
}

pub(crate) fn missing(expected: ExpectedSyntax, span: Span) -> Event {
    Event::Missing(MissingSyntax { expected, span })
}

pub(crate) struct BuiltTree {
    nodes: Vec<SyntaxNode>,
    elements: Vec<SyntaxElement>,
    root: NodeId,
}

pub(crate) fn build_tree(source: SourceId, events: &[Event]) -> BuiltTree {
    struct Frame {
        kind: NodeKind,
        offset: ByteOffset,
        children: Vec<SyntaxElement>,
    }

    let mut stack: Vec<Frame> = Vec::new();
    let mut nodes = Vec::new();
    let mut elements = Vec::new();
    let mut root = None;
    let mut consumed_starts = vec![false; events.len()];

    for (event_index, event) in events.iter().copied().enumerate() {
        match event {
            Event::Start { .. } if consumed_starts[event_index] => {}
            Event::Start { .. } => {
                let mut chain = Vec::new();
                let mut current = event_index;
                loop {
                    assert!(
                        !consumed_starts[current],
                        "forward-parent cycle in event stream"
                    );
                    consumed_starts[current] = true;
                    let Event::Start {
                        kind,
                        offset,
                        forward_parent,
                    } = events[current]
                    else {
                        panic!("forward parent must point to a start event");
                    };
                    chain.push((kind, offset));
                    let Some(distance) = forward_parent else {
                        break;
                    };
                    assert!(
                        distance > 0,
                        "forward parent must advance in the event stream"
                    );
                    current = current
                        .checked_add(distance)
                        .filter(|index| *index < events.len())
                        .expect("forward parent escaped the event stream");
                }

                for (kind, offset) in chain.into_iter().rev() {
                    stack.push(Frame {
                        kind,
                        offset,
                        children: Vec::new(),
                    });
                }
            }
            Event::Token(token) => stack
                .last_mut()
                .expect("token event requires an open node")
                .children
                .push(SyntaxElement::Token(token)),
            Event::Missing(missing) => stack
                .last_mut()
                .expect("missing event requires an open node")
                .children
                .push(SyntaxElement::Missing(missing)),
            Event::Finish => {
                let frame = stack.pop().expect("finish event requires an open node");
                let range = child_range(&frame.children, &nodes)
                    .unwrap_or_else(|| TextRange::empty(frame.offset));
                let first_child = elements.len();
                let child_count = frame.children.len();
                elements.extend(frame.children);
                let id = NodeId::new(source, nodes.len());
                let node = SyntaxNode {
                    kind: frame.kind,
                    range,
                    first_child,
                    child_count,
                };
                nodes.push(node);
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(SyntaxElement::Node(id));
                } else {
                    assert!(
                        root.replace(id).is_none(),
                        "event stream produced two roots"
                    );
                }
            }
        }
    }

    assert!(stack.is_empty(), "event stream left nodes open");
    BuiltTree {
        nodes,
        elements,
        root: root.expect("event stream did not produce a root"),
    }
}

fn child_range(children: &[SyntaxElement], nodes: &[SyntaxNode]) -> Option<TextRange> {
    let first = children
        .first()
        .map(|element| element_range(element, nodes))?;
    let last = children
        .last()
        .map(|element| element_range(element, nodes))?;
    Some(TextRange::new(first.start(), last.end()))
}

fn element_range(element: &SyntaxElement, nodes: &[SyntaxNode]) -> TextRange {
    match element {
        SyntaxElement::Node(id) => nodes[id.index()].range(),
        SyntaxElement::Token(token) => token.range(),
        SyntaxElement::Missing(missing) => missing.span().range(),
    }
}
