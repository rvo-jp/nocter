use std::collections::BTreeSet;

use super::ContextualSpelling;

#[test]
fn shared_catalog_matches_the_versioned_grammar_tables() {
    let grammar = include_str!("../../../../../../spec/language/syntactic-grammar.md");
    let development_grammar = include_str!("../../../../../spec/language/syntactic-grammar.md");
    let specified: BTreeSet<_> = [grammar, development_grammar]
        .into_iter()
        .flat_map(contextual_spellings)
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

fn contextual_spellings(grammar: &str) -> impl Iterator<Item = &str> {
    let section = grammar
        .split_once("## Contextual Spellings\n")
        .unwrap()
        .1
        .split_once("\n## ")
        .unwrap()
        .0;
    section
        .lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|line| line.split_once('`').map(|(spelling, _)| spelling))
}
