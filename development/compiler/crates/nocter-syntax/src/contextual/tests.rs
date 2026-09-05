use std::collections::BTreeSet;

use super::ContextualSpelling;

#[test]
fn catalog_matches_the_normative_grammar_table() {
    let grammar = include_str!("../../../../../../spec/25-syntactic-grammar.md");
    let section = grammar
        .split_once("## Contextual Spellings\n")
        .unwrap()
        .1
        .split_once("\n## ")
        .unwrap()
        .0;
    let specified: BTreeSet<_> = section
        .lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|line| line.split_once('`').map(|(spelling, _)| spelling))
        .collect();
    let implemented: BTreeSet<_> = ContextualSpelling::ALL
        .iter()
        .map(|spelling| spelling.as_str())
        .collect();

    assert_eq!(implemented, specified);
    assert_eq!(implemented.len(), ContextualSpelling::ALL.len());
    for spelling in ContextualSpelling::ALL {
        assert_eq!(
            ContextualSpelling::from_spelling(spelling.as_str()),
            Some(*spelling)
        );
    }
}
