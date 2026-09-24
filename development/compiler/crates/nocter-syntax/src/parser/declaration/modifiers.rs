use super::super::Parser;
use crate::{Keyword, NodeKind, TokenKind};

/// The modifier vocabulary admitted before one callable-shaped declaration head.
///
/// This is a syntactic boundary only. It decides which tokens can occur in each grammar position;
/// declaration lowering remains the sole owner of their semantic contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::parser) enum CallablePrefixGrammar {
    /// `const noalloc blocking async`, used by ordinary functions and methods.
    DeferredAllowed,
    /// `const noalloc blocking`, used where deferred execution has no production.
    Immediate,
    /// `noalloc`, used by implicit call surfaces and destruction.
    NoAllocationOnly,
}

impl CallablePrefixGrammar {
    fn admits(self, modifier: CallableModifier) -> bool {
        match self {
            Self::DeferredAllowed => true,
            Self::Immediate => modifier != CallableModifier::Async,
            Self::NoAllocationOnly => modifier == CallableModifier::NoAllocation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallableModifier {
    CompileTime,
    NoAllocation,
    Blocking,
    Async,
}

impl CallableModifier {
    const ORDER: [Self; 4] = [
        Self::CompileTime,
        Self::NoAllocation,
        Self::Blocking,
        Self::Async,
    ];

    const fn keyword(self) -> Keyword {
        match self {
            Self::CompileTime => Keyword::Const,
            Self::NoAllocation => Keyword::NoAlloc,
            Self::Blocking => Keyword::Blocking,
            Self::Async => Keyword::Async,
        }
    }

    const fn node_kind(self) -> NodeKind {
        match self {
            Self::CompileTime => NodeKind::CompileTimeModifier,
            Self::NoAllocation => NodeKind::NoAllocationModifier,
            Self::Blocking => NodeKind::BlockingModifier,
            Self::Async => NodeKind::AsyncModifier,
        }
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
    for modifier in CallableModifier::ORDER {
        if grammar.admits(modifier)
            && parser.tokens[end].kind() == TokenKind::Keyword(modifier.keyword())
        {
            end += 1;
        }
    }
    CallablePrefix { start, end }
}

/// Consumes the exact canonical prefix recognized for this grammar and retains dedicated nodes.
pub(in crate::parser) fn parse(parser: &mut Parser<'_>, grammar: CallablePrefixGrammar) {
    for modifier in CallableModifier::ORDER {
        if grammar.admits(modifier) && parser.at_keyword(modifier.keyword()) {
            let marker = parser.start();
            parser.bump();
            parser.complete(marker, modifier.node_kind());
        }
    }
}
