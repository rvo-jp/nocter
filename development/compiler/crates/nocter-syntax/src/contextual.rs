//! Closed vocabulary for identifier-shaped contextual spellings.

macro_rules! contextual_spellings {
    ($($variant:ident => $text:literal),+ $(,)?) => {
        /// Identifier-shaped spellings with a grammar- or semantic-owned contextual role.
        ///
        /// The lexer deliberately keeps these out of `Keyword`. Parser and tooling clients use
        /// this closed catalog instead of independently comparing source text.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum ContextualSpelling {
            $($variant),+
        }

        impl ContextualSpelling {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub fn from_spelling(text: &str) -> Option<Self> {
                match text {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)+
                }
            }
        }
    };
}

contextual_spellings! {
    Copy => "copy",
    Where => "where",
    Some => "some",
    From => "from",
    Default => "default",
    Coerce => "coerce",
    Drop => "drop",
    Static => "static",
    LowerSelf => "self",
    UpperSelf => "Self",
    Error => "error",
    Discard => "_",
    Target => "target",
}

#[cfg(test)]
mod tests;
