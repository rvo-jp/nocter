use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Recipe(u32);

#[test]
fn recipe_equality_never_substitutes_for_evaluated_index_evidence() {
    let root = ValuePath::<Recipe>::default();
    let first = root.project(ValueProjection::Array(Recipe(0)));
    let other = root.project(ValueProjection::Array(Recipe(1)));
    assert_eq!(first, first.clone());
    // A recipe may run again after a mutation or loop edge; even equal recipe keys need evidence.
    assert_eq!(
        first.relation(&first, |_, _| IndexRelation::Unknown),
        PathRelation::MayOverlap
    );
    assert_eq!(
        first.relation(&other, |_, _| IndexRelation::Unknown),
        PathRelation::MayOverlap
    );
    assert_eq!(
        first.relation(&other, |_, _| IndexRelation::Equal),
        PathRelation::Equal
    );
    assert_eq!(
        first.relation(&other, |_, _| IndexRelation::Distinct),
        PathRelation::Disjoint
    );
    assert_eq!(
        root.relation(&first, |_, _| panic!(
            "root coverage needs no index relation"
        )),
        PathRelation::Ancestor
    );
}

#[test]
fn unknown_indices_can_be_separated_by_later_disjoint_components() {
    let root = ValuePath::<Recipe>::default();
    let first = root.project(ValueProjection::Array(Recipe(0)));
    let other = root.project(ValueProjection::Array(Recipe(1)));
    let left = first.project(ValueProjection::Tuple(0));
    let right = other.project(ValueProjection::Tuple(1));
    let same_field = other.project(ValueProjection::Tuple(0));
    assert_eq!(
        left.relation(&right, |_, _| IndexRelation::Unknown),
        PathRelation::Disjoint
    );
    assert_eq!(
        left.relation(&same_field, |_, _| IndexRelation::Unknown),
        PathRelation::MayOverlap
    );
    assert_eq!(
        first.relation(&same_field, |_, _| IndexRelation::Unknown),
        PathRelation::MayOverlap
    );
    assert_eq!(
        first.relation(&same_field, |_, _| IndexRelation::Equal),
        PathRelation::Ancestor
    );
    assert_eq!(
        same_field.relation(&first, |_, _| IndexRelation::Equal),
        PathRelation::Descendant
    );
    assert_eq!(
        left.relation(&ValuePath::new([ValueProjection::Tuple(1)]), |_, _| {
            IndexRelation::Distinct
        }),
        PathRelation::MayOverlap
    );
    assert_eq!(
        first.concat(&ValuePath::new([ValueProjection::Tuple(0)])),
        left
    );
}

/// Independent finite-region interpretation of a homogeneous two-by-two array of pairs.
fn region(path: &ValuePath<bool>, indices: [u64; 2]) -> BTreeSet<(u64, u64, usize)> {
    let mut result = BTreeSet::new();
    for outer in 0..2 {
        for inner in 0..2 {
            for field in 0..2 {
                if path.fields().iter().enumerate().all(|(depth, projection)| {
                    match (depth, projection) {
                        (0, ValueProjection::Array(key)) => outer == indices[usize::from(*key)],
                        (1, ValueProjection::Array(key)) => inner == indices[usize::from(*key)],
                        (2, ValueProjection::Tuple(index)) => field == *index,
                        _ => false,
                    }
                }) {
                    result.insert((outer, inner, field));
                }
            }
        }
    }
    result
}

fn paths() -> Vec<ValuePath<bool>> {
    let root = ValuePath::default();
    let mut result = vec![root.clone()];
    for outer in [false, true] {
        let row = root.project(ValueProjection::Array(outer));
        result.push(row.clone());
        for inner in [false, true] {
            let pair = row.project(ValueProjection::Array(inner));
            result.push(pair.clone());
            for field in 0..2 {
                result.push(pair.project(ValueProjection::Tuple(field)));
            }
        }
    }
    result
}

#[test]
fn symbolic_relations_are_sound_for_every_small_concrete_index_assignment() {
    let paths = paths();
    for stable_keys in [false, true] {
        for left in &paths {
            for right in &paths {
                let relation = left.relation(right, |a, b| {
                    if stable_keys && a == b {
                        IndexRelation::Equal
                    } else {
                        IndexRelation::Unknown
                    }
                });
                for assignment in [[0, 0], [0, 1], [1, 0], [1, 1]] {
                    let a = region(left, assignment);
                    let b = region(right, assignment);
                    let sound = match relation {
                        PathRelation::Equal => a == b,
                        PathRelation::Ancestor => b.is_subset(&a),
                        PathRelation::Descendant => a.is_subset(&b),
                        PathRelation::Disjoint => a.is_disjoint(&b),
                        PathRelation::MayOverlap => true,
                    };
                    assert!(sound, "{left:?} {right:?} {assignment:?}: {relation:?}");
                }
            }
        }
    }
}

#[test]
fn closed_index_relations_match_exact_logical_region_sets() {
    let paths = paths();
    for left in &paths {
        for right in &paths {
            for assignment in [[0, 0], [0, 1], [1, 0], [1, 1]] {
                let a = region(left, assignment);
                let b = region(right, assignment);
                let expected = if a == b {
                    PathRelation::Equal
                } else if a.is_disjoint(&b) {
                    PathRelation::Disjoint
                } else if b.is_subset(&a) {
                    PathRelation::Ancestor
                } else if a.is_subset(&b) {
                    PathRelation::Descendant
                } else {
                    PathRelation::MayOverlap
                };
                let actual = left.relation(right, |a, b| {
                    if assignment[usize::from(*a)] == assignment[usize::from(*b)] {
                        IndexRelation::Equal
                    } else {
                        IndexRelation::Distinct
                    }
                });
                assert_eq!(actual, expected, "{left:?} {right:?} {assignment:?}");
            }
        }
    }
}
