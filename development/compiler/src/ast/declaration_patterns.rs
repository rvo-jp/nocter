use super::{TypeExpr, WhereClause};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Term {
    Variable(Side, String),
    Node(String, Vec<Term>),
}

pub(crate) fn declaration_patterns_overlap<L, R>(
    left_types: &[&TypeExpr],
    left_parameters: L,
    left_clause: Option<&WhereClause>,
    right_types: &[&TypeExpr],
    right_parameters: R,
    right_clause: Option<&WhereClause>,
) -> bool
where
    L: IntoIterator,
    L::Item: AsRef<str>,
    R: IntoIterator,
    R::Item: AsRef<str>,
{
    if left_types.len() != right_types.len() {
        return false;
    }
    let left_parameters = left_parameters
        .into_iter()
        .map(|name| name.as_ref().to_string())
        .collect::<HashSet<_>>();
    let right_parameters = right_parameters
        .into_iter()
        .map(|name| name.as_ref().to_string())
        .collect::<HashSet<_>>();
    let left_refinements = refinements(left_clause);
    let right_refinements = refinements(right_clause);
    let left = Term::Node(
        "tuple".to_string(),
        left_types
            .iter()
            .map(|ty| {
                term(
                    ty,
                    Side::Left,
                    &left_parameters,
                    &left_refinements,
                    &mut HashSet::new(),
                )
            })
            .collect(),
    );
    let right = Term::Node(
        "tuple".to_string(),
        right_types
            .iter()
            .map(|ty| {
                term(
                    ty,
                    Side::Right,
                    &right_parameters,
                    &right_refinements,
                    &mut HashSet::new(),
                )
            })
            .collect(),
    );
    unify(left, right, &mut HashMap::new())
}

fn refinements(clause: Option<&WhereClause>) -> HashMap<String, &TypeExpr> {
    clause
        .into_iter()
        .flat_map(WhereClause::refinements)
        .map(|refinement| (refinement.name.clone(), &refinement.value))
        .collect()
}

fn term(
    ty: &TypeExpr,
    side: Side,
    parameters: &HashSet<String>,
    refinements: &HashMap<String, &TypeExpr>,
    expanding: &mut HashSet<String>,
) -> Term {
    if let TypeExpr::Reference(reference) = ty
        && parameters.contains(&reference.name)
    {
        if let Some(value) = refinements.get(&reference.name)
            && expanding.insert(reference.name.clone())
        {
            let result = term(value, side, parameters, refinements, expanding);
            expanding.remove(&reference.name);
            return result;
        }
        return Term::Variable(side, reference.name.clone());
    }
    match ty {
        TypeExpr::Callable(callable) => {
            let mut children = callable
                .parameters
                .iter()
                .map(|parameter| term(&parameter.ty, side, parameters, refinements, expanding))
                .collect::<Vec<_>>();
            children.push(term(
                &callable.return_type,
                side,
                parameters,
                refinements,
                expanding,
            ));
            Term::Node(format!("callable:{:?}", callable.capability), children)
        }
        TypeExpr::Closure(closure) => Term::Node(closure.identity_name(), Vec::new()),
        TypeExpr::Reference(reference) => Term::Node(format!("ref:{}", reference.name), Vec::new()),
        TypeExpr::Generic(generic) => Term::Node(
            format!("generic:{}", generic.name),
            generic
                .arguments
                .iter()
                .map(|argument| term(argument, side, parameters, refinements, expanding))
                .collect(),
        ),
        TypeExpr::Projection(projection) => Term::Node(
            format!("projection:{}", projection.name),
            vec![term(
                &projection.base,
                side,
                parameters,
                refinements,
                expanding,
            )],
        ),
        TypeExpr::Pointer(pointer) => Term::Node(
            "pointer".to_string(),
            vec![term(
                &pointer.inner,
                side,
                parameters,
                refinements,
                expanding,
            )],
        ),
        TypeExpr::Borrow(borrow) => Term::Node(
            format!("borrow:{}", borrow.is_readwrite),
            vec![term(
                &borrow.inner,
                side,
                parameters,
                refinements,
                expanding,
            )],
        ),
        TypeExpr::View(view) => Term::Node(
            format!("view:{}", view.is_readwrite),
            vec![term(
                &view.element,
                side,
                parameters,
                refinements,
                expanding,
            )],
        ),
        TypeExpr::Array(array) => Term::Node(
            format!("array:{}", array.length.value),
            vec![term(
                &array.element,
                side,
                parameters,
                refinements,
                expanding,
            )],
        ),
        TypeExpr::Optional(optional) => Term::Node(
            "optional".to_string(),
            vec![term(
                &optional.inner,
                side,
                parameters,
                refinements,
                expanding,
            )],
        ),
        TypeExpr::Fallible(fallible) => Term::Node(
            "fallible".to_string(),
            vec![
                term(&fallible.success, side, parameters, refinements, expanding),
                term(&fallible.error, side, parameters, refinements, expanding),
            ],
        ),
    }
}

fn unify(left: Term, right: Term, substitutions: &mut HashMap<(Side, String), Term>) -> bool {
    let left = resolve(left, substitutions);
    let right = resolve(right, substitutions);
    if left == right {
        return true;
    }
    match (left, right) {
        (Term::Variable(side, name), value) | (value, Term::Variable(side, name)) => {
            let key = (side, name);
            if occurs(&key, &value, substitutions) {
                return false;
            }
            substitutions.insert(key, value);
            true
        }
        (Term::Node(left_name, left), Term::Node(right_name, right)) => {
            left_name == right_name
                && left.len() == right.len()
                && left
                    .into_iter()
                    .zip(right)
                    .all(|(left, right)| unify(left, right, substitutions))
        }
    }
}

fn resolve(term: Term, substitutions: &HashMap<(Side, String), Term>) -> Term {
    let mut current = term;
    let mut seen = HashSet::new();
    while let Term::Variable(side, name) = &current {
        let key = (*side, name.clone());
        if !seen.insert(key.clone()) {
            break;
        }
        let Some(value) = substitutions.get(&key) else {
            break;
        };
        current = value.clone();
    }
    current
}

fn occurs(
    variable: &(Side, String),
    term: &Term,
    substitutions: &HashMap<(Side, String), Term>,
) -> bool {
    match resolve(term.clone(), substitutions) {
        Term::Variable(side, name) => variable == &(side, name),
        Term::Node(_, children) => children
            .iter()
            .any(|child| occurs(variable, child, substitutions)),
    }
}
