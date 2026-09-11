/// Closed set of member names exposed by compiler-owned argument-pack values.
///
/// Argument packs are not nominal values and therefore cannot obtain their surface from an
/// ordinary `instance` declaration. Keeping their members here prevents semantic checking from
/// assigning compiler behavior to an untyped source spelling.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArgumentPackMember {
    Len,
}

impl ArgumentPackMember {
    pub const ALL: &'static [Self] = &[Self::Len];

    #[must_use]
    pub fn from_spelling(text: &str) -> Option<Self> {
        match text {
            "len" => Some(Self::Len),
            _ => None,
        }
    }

    #[must_use]
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Len => "len",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::ArgumentPackMember;

    #[test]
    fn spellings_are_unique_and_round_trip() {
        let spellings = ArgumentPackMember::ALL
            .iter()
            .map(|member| member.spelling())
            .collect::<BTreeSet<_>>();

        assert_eq!(spellings.len(), ArgumentPackMember::ALL.len());
        for member in ArgumentPackMember::ALL {
            assert_eq!(
                ArgumentPackMember::from_spelling(member.spelling()),
                Some(*member)
            );
        }
        assert_eq!(ArgumentPackMember::from_spelling("capacity"), None);
    }
}
