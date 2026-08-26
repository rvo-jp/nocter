use nocter_model::BuiltinType;
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxTree, TokenKind};

#[derive(Clone, Copy)]
pub(crate) struct IntegerSpec {
    pub(crate) signed: bool,
    pub(crate) bits: u32,
    pub(crate) minimum: i128,
    pub(crate) maximum: i128,
}

impl IntegerSpec {
    pub(crate) const fn contains(self, value: i128) -> bool {
        value >= self.minimum && value <= self.maximum
    }
}

pub(crate) fn integer_spec(builtin: BuiltinType) -> Option<IntegerSpec> {
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

pub(crate) fn parse_integer(text: &str) -> Option<u64> {
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

pub(crate) fn shift(
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

pub(crate) fn expression_children(tree: &SyntaxTree, node: NodeId) -> Vec<NodeId> {
    nocter_syntax::child_nodes(tree, node)
        .into_iter()
        .filter(|node| {
            tree.node(*node).is_some_and(|syntax| {
                syntax.kind() != NodeKind::Type && syntax.kind() != NodeKind::MemberSuffix
            })
        })
        .collect()
}

pub(crate) fn one_expression_child(tree: &SyntaxTree, node: NodeId) -> Option<NodeId> {
    let children = expression_children(tree, node);
    (children.len() == 1).then_some(children[0])
}

pub(crate) fn direct_punctuation(tree: &SyntaxTree, node: NodeId) -> Option<Punctuation> {
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
