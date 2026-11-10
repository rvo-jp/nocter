//! One memoization and active-path authority for contracts, typed bodies and required values.
use super::Completed;
use crate::value_access::Query;
use nocter_declarations::value_access::DeclarationId;
use nocter_dependency_query::DependencyQuery;
use nocter_model::AccessCallableId;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Request {
    Header(DeclarationId),
    Body(AccessCallableId),
    Value(AccessCallableId),
}
#[derive(Clone, Debug)]
pub(super) enum Fact {
    Header,
    Body(Box<Query>),
    Value(Completed),
}
pub(super) type Dependencies = DependencyQuery<Request, Fact>;

pub(super) fn body(query: &Dependencies, id: AccessCallableId) -> Option<&Query> {
    match query.completed(&Request::Body(id))? {
        Fact::Body(body) => Some(body),
        _ => None,
    }
}
pub(super) fn value(query: &Dependencies, id: AccessCallableId) -> Option<&Completed> {
    match query.completed(&Request::Value(id))? {
        Fact::Value(value) => Some(value),
        _ => None,
    }
}
/// Pending header/body work participates in cycle detection, not source execution-call depth.
pub(super) fn value_depth(query: &Dependencies) -> usize {
    query
        .active_keys()
        .filter(|key| matches!(key, Request::Value(_)))
        .count()
}
