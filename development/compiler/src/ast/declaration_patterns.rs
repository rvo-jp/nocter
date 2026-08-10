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
    declaration_patterns_overlap_with_names(
        left_types,
        left_parameters,
        left_clause,
        right_types,
        right_parameters,
        right_clause,
        &str::to_string,
    )
}

pub(crate) fn declaration_patterns_overlap_with_names<L, R>(
    left_types: &[&TypeExpr],
    left_parameters: L,
    left_clause: Option<&WhereClause>,
    right_types: &[&TypeExpr],
    right_parameters: R,
    right_clause: Option<&WhereClause>,
    normalize_name: &impl Fn(&str) -> String,
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
                TermBuilder::new(
                    Side::Left,
                    &left_parameters,
                    &left_refinements,
                    normalize_name,
                )
                .term(ty)
            })
            .collect(),
    );
    let right = Term::Node(
        "tuple".to_string(),
        right_types
            .iter()
            .map(|ty| {
                TermBuilder::new(
                    Side::Right,
                    &right_parameters,
                    &right_refinements,
                    normalize_name,
                )
                .term(ty)
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

struct TermBuilder<'a, F> {
    side: Side,
    parameters: &'a HashSet<String>,
    refinements: &'a HashMap<String, &'a TypeExpr>,
    expanding: HashSet<String>,
    normalize_name: &'a F,
}

impl<'a, F: Fn(&str) -> String> TermBuilder<'a, F> {
    fn new(
        side: Side,
        parameters: &'a HashSet<String>,
        refinements: &'a HashMap<String, &'a TypeExpr>,
        normalize_name: &'a F,
    ) -> Self {
        Self {
            side,
            parameters,
            refinements,
            expanding: HashSet::new(),
            normalize_name,
        }
    }

    fn term(&mut self, ty: &TypeExpr) -> Term {
        if let TypeExpr::Reference(reference) = ty
            && self.parameters.contains(&reference.name)
        {
            if let Some(value) = self.refinements.get(&reference.name)
                && self.expanding.insert(reference.name.clone())
            {
                let result = self.term(value);
                self.expanding.remove(&reference.name);
                return result;
            }
            return Term::Variable(self.side, reference.name.clone());
        }
        match ty {
            TypeExpr::Callable(callable) => {
                let mut children = callable
                    .parameters
                    .iter()
                    .map(|parameter| self.term(&parameter.ty))
                    .collect::<Vec<_>>();
                children.push(self.term(&callable.return_type));
                Term::Node(format!("callable:{:?}", callable.capability), children)
            }
            TypeExpr::Closure(closure) => Term::Node(closure.identity_name(), Vec::new()),
            TypeExpr::Opaque(opaque) => {
                let mut children = vec![self.term(&opaque.interface)];
                children.extend(
                    opaque
                        .associated_bindings
                        .iter()
                        .map(|binding| self.term(&binding.value)),
                );
                Term::Node("opaque".to_string(), children)
            }
            TypeExpr::Reference(reference) => Term::Node(
                format!("ref:{}", (self.normalize_name)(&reference.name)),
                Vec::new(),
            ),
            TypeExpr::Generic(generic) => Term::Node(
                format!("generic:{}", (self.normalize_name)(&generic.name)),
                generic
                    .arguments
                    .iter()
                    .map(|argument| self.term(argument))
                    .collect(),
            ),
            TypeExpr::Projection(projection) => Term::Node(
                format!("projection:{}", projection.name),
                vec![self.term(&projection.base)],
            ),
            TypeExpr::Pointer(pointer) => {
                Term::Node("pointer".to_string(), vec![self.term(&pointer.inner)])
            }
            TypeExpr::Borrow(borrow) => Term::Node(
                format!("borrow:{}", borrow.is_readwrite),
                vec![self.term(&borrow.inner)],
            ),
            TypeExpr::View(view) => Term::Node(
                format!("view:{}", view.is_readwrite),
                vec![self.term(&view.element)],
            ),
            TypeExpr::Array(array) => Term::Node(
                format!("array:{}", array.length.value),
                vec![self.term(&array.element)],
            ),
            TypeExpr::Optional(optional) => {
                Term::Node("optional".to_string(), vec![self.term(&optional.inner)])
            }
            TypeExpr::Fallible(fallible) => Term::Node(
                "fallible".to_string(),
                vec![self.term(&fallible.success), self.term(&fallible.error)],
            ),
        }
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
