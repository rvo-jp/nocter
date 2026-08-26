use nocter_model::TypeId;

/// Specialized semantic work retained by a machine primitive target.
///
/// A no-op destruction dependency explicitly records that the subject is copyable. Nontrivial
/// destruction is already an ordinary direct machine call, so recursive plans cannot cross this
/// boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachinePrimitiveDependency {
    None,
    NoopDestruction { subject: TypeId },
}
