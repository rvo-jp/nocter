use nocter_source::{ByteOffset, SourceId, Span, TextRange};

use crate::{ExpectedSyntax, LexedFile, ParseDiagnostic, Token, TokenKind};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NodeKind {
    PackageFile,
    ModuleSource,
    PackageDirective,
    DirectiveValue,
    DirectiveRecord,
    DirectiveField,
    StringLiteral,
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
    ConstructDeclaration,
    ConstructionFunction,
    LiteralDeclaration,
    LiteralShape,
    LiteralParameters,
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
    CallableTail,
    ProvenanceClause,
    Block,
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

/// Stable identity of a node inside one immutable syntax tree.
///
/// Nodes and child elements live in flat arenas. This keeps ownership non-recursive even when a
/// valid source contains a very deep chain of prefix types or expressions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(usize);

impl NodeId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
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
    lexical: TokenId,
    kind: TokenKind,
    range: TextRange,
}

impl SyntaxToken {
    pub(crate) const fn new(lexical: TokenId, kind: TokenKind, range: TextRange) -> Self {
        Self {
            lexical,
            kind,
            range,
        }
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

#[derive(Debug)]
pub struct SyntaxTree {
    lexed: LexedFile,
    nodes: Vec<SyntaxNode>,
    elements: Vec<SyntaxElement>,
    root: NodeId,
    diagnostics: Vec<ParseDiagnostic>,
}

impl SyntaxTree {
    pub(crate) fn new(
        lexed: LexedFile,
        built: BuiltTree,
        diagnostics: Vec<ParseDiagnostic>,
    ) -> Self {
        Self {
            lexed,
            nodes: built.nodes,
            elements: built.elements,
            root: built.root,
            diagnostics,
        }
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
        self.nodes.get(id.index())
    }

    #[must_use]
    pub fn children(&self, id: NodeId) -> &[SyntaxElement] {
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
}

pub(crate) enum Event {
    Start { kind: NodeKind, offset: ByteOffset },
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

pub(crate) fn build_tree(events: Vec<Event>) -> BuiltTree {
    struct Frame {
        kind: NodeKind,
        offset: ByteOffset,
        children: Vec<SyntaxElement>,
    }

    let mut stack: Vec<Frame> = Vec::new();
    let mut nodes = Vec::new();
    let mut elements = Vec::new();
    let mut root = None;

    for event in events {
        match event {
            Event::Start { kind, offset } => stack.push(Frame {
                kind,
                offset,
                children: Vec::new(),
            }),
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
                let id = NodeId(nodes.len());
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
