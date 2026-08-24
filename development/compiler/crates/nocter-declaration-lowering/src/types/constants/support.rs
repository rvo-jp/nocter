use nocter_model::BuiltinType;
use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

#[derive(Clone, Copy)]
pub(super) struct IntegerSpec {
    pub(super) signed: bool,
    pub(super) bits: u32,
    minimum: i128,
    pub(super) maximum: i128,
}

impl IntegerSpec {
    pub(super) const fn contains(self, value: i128) -> bool {
        value >= self.minimum && value <= self.maximum
    }
}

pub(super) fn integer_spec(builtin: BuiltinType) -> Option<IntegerSpec> {
    let (signed, bits) = match builtin {
        BuiltinType::I8 => (true, 8),
        BuiltinType::I16 => (true, 16),
        BuiltinType::I32 => (true, 32),
        BuiltinType::I64 | BuiltinType::Isize => (true, 64),
        BuiltinType::U8 => (false, 8),
        BuiltinType::U16 => (false, 16),
        BuiltinType::U32 => (false, 32),
        BuiltinType::U64 | BuiltinType::Usize => (false, 64),
        _ => return None,
    };
    let (minimum, maximum) = if signed {
        (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
    } else {
        (0, (1_i128 << bits) - 1)
    };
    Some(IntegerSpec {
        signed,
        bits,
        minimum,
        maximum,
    })
}

pub(super) fn parse_integer(text: &str) -> Option<u64> {
    let compact = text
        .chars()
        .filter(|character| *character != '_')
        .collect::<String>();
    if let Some(digits) = compact.strip_prefix("0x") {
        u64::from_str_radix(digits, 16).ok()
    } else if let Some(digits) = compact.strip_prefix("0b") {
        u64::from_str_radix(digits, 2).ok()
    } else {
        compact.parse().ok()
    }
}

pub(super) fn shift(
    left: i128,
    right: i128,
    operator: Punctuation,
    spec: IntegerSpec,
) -> Option<i128> {
    let count = u32::try_from(right).ok()?;
    if count >= spec.bits {
        return None;
    }
    if operator == Punctuation::ShiftRight {
        return Some(if spec.signed {
            left >> count
        } else {
            (left.cast_unsigned() >> count).cast_signed()
        });
    }
    let mask = (1_u128 << spec.bits) - 1;
    let bits = (left.cast_unsigned() << count) & mask;
    if spec.signed && bits & (1_u128 << (spec.bits - 1)) != 0 {
        Some(bits.cast_signed() - (1_i128 << spec.bits))
    } else {
        Some(bits.cast_signed())
    }
}

pub(super) fn direct_node(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child)
                if tree.node(*child).is_some_and(|node| node.kind() == kind) =>
            {
                Some(*child)
            }
            _ => None,
        })
}

pub(super) fn direct_nodes(tree: &SyntaxTree, node: NodeId) -> Vec<NodeId> {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(node) => Some(*node),
            _ => None,
        })
        .collect()
}

pub(super) fn expression_children(tree: &SyntaxTree, node: NodeId) -> Vec<NodeId> {
    direct_nodes(tree, node)
        .into_iter()
        .filter(|node| {
            tree.node(*node).is_some_and(|syntax| {
                syntax.kind() != NodeKind::Type && syntax.kind() != NodeKind::MemberSuffix
            })
        })
        .collect()
}

pub(super) fn one_expression_child(tree: &SyntaxTree, node: NodeId) -> Option<NodeId> {
    let children = expression_children(tree, node);
    (children.len() == 1).then_some(children[0])
}

pub(super) fn direct_token(tree: &SyntaxTree, node: NodeId) -> Option<SyntaxToken> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => Some(*token),
            _ => None,
        })
}

pub(super) fn direct_punctuation(tree: &SyntaxTree, node: NodeId) -> Option<Punctuation> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => match token.kind() {
                TokenKind::Punctuation(punctuation) => Some(punctuation),
                _ => None,
            },
            _ => None,
        })
}
