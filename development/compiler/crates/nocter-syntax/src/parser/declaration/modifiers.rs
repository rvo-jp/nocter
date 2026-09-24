use super::super::Parser;
use crate::{Keyword, NodeKind, TokenKind};
use nocter_language::CallableModifier;

pub(in crate::parser) use nocter_language::CallablePrefixGrammar;

const fn modifier_node_kind(modifier: CallableModifier) -> NodeKind {
    match modifier {
        CallableModifier::CompileTime => NodeKind::CompileTimeModifier,
        CallableModifier::NoAllocation => NodeKind::NoAllocationModifier,
        CallableModifier::NoTrap => NodeKind::NoTrapModifier,
        CallableModifier::Blocking => NodeKind::BlockingModifier,
        CallableModifier::Async => NodeKind::AsyncModifier,
    }
}

/// One non-mutating recognition of a canonical callable prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::parser) struct CallablePrefix {
    start: usize,
    end: usize,
}

impl CallablePrefix {
    #[must_use]
    pub(in crate::parser) const fn end(self) -> usize {
        self.end
    }

    #[must_use]
    pub(in crate::parser) const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Recognizes the canonical prefix without producing parser events.
///
/// Declaration classification and actual parsing call this same authority, so adding or reordering
/// a modifier cannot leave their lookahead grammars inconsistent.
#[must_use]
pub(in crate::parser) fn scan(
    parser: &Parser<'_>,
    start: usize,
    grammar: CallablePrefixGrammar,
) -> CallablePrefix {
    let mut end = start;
    for modifier in CallableModifier::ALL.iter().copied() {
        if grammar.admits(modifier)
            && parser.tokens[end].kind() == TokenKind::Keyword(Keyword::from(modifier))
        {
            end += 1;
        }
    }
    CallablePrefix { start, end }
}

/// Consumes the exact canonical prefix recognized for this grammar and retains dedicated nodes.
pub(in crate::parser) fn parse(parser: &mut Parser<'_>, grammar: CallablePrefixGrammar) {
    for modifier in CallableModifier::ALL.iter().copied() {
        if grammar.admits(modifier) && parser.at_keyword(Keyword::from(modifier)) {
            let marker = parser.start();
            parser.bump();
            parser.complete(marker, modifier_node_kind(modifier));
        }
    }
}
