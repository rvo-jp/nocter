use std::collections::BTreeSet;

use super::BuiltinType;

#[test]
fn builtin_type_spellings_are_closed_and_round_trip() {
    let spellings: BTreeSet<_> = BuiltinType::ALL
        .iter()
        .map(|builtin| builtin.as_str())
        .collect();

    assert_eq!(spellings.len(), BuiltinType::ALL.len());
    for builtin in BuiltinType::ALL {
        assert_eq!(BuiltinType::from_spelling(builtin.as_str()), Some(*builtin));
    }
    assert_eq!(BuiltinType::from_spelling("String"), None);
}
