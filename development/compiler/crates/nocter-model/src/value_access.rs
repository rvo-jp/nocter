use crate::TypeId;

/// Source-independent operation permission. This is not a borrow capability or a coercion rank.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueAccessMode {
    Read,
    Update,
    Own,
}

/// Transient operation access to one resolved data shape, not a qualified semantic data type.
///
/// The owner of the surrounding semantic product validates `data` against its type authority.
/// This descriptor does not flatten an outcome of access into access to an outcome, grant Copy
/// evidence, or describe a physical pointer/storage layout.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueAccess {
    data: TypeId,
    mode: ValueAccessMode,
}

impl ValueAccess {
    #[must_use]
    pub const fn new(data: TypeId, mode: ValueAccessMode) -> Self {
        Self { data, mode }
    }

    #[must_use]
    pub const fn data(self) -> TypeId {
        self.data
    }

    #[must_use]
    pub const fn mode(self) -> ValueAccessMode {
        self.mode
    }
}

/// Operation selected by a callable input, independently of the input's data type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueInputOperation {
    Read,
    Edit,
    Take,
}
impl ValueInputOperation {
    #[must_use]
    pub const fn access_mode(self) -> ValueAccessMode {
        match self {
            Self::Read => ValueAccessMode::Read,
            Self::Edit => ValueAccessMode::Update,
            Self::Take => ValueAccessMode::Own,
        }
    }
    #[must_use]
    pub const fn binding_kind(self) -> ValueBindingKind {
        match self {
            Self::Read => ValueBindingKind::ReadAlias,
            Self::Edit => ValueBindingKind::EditAlias,
            Self::Take => ValueBindingKind::MutableValue,
        }
    }
}

/// How a local stores a value or accesses a separate existing place.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueBindingKind {
    FixedValue,
    MutableValue,
    ReadAlias,
    EditAlias,
}
impl ValueBindingKind {
    #[must_use]
    pub const fn access_mode(self) -> ValueAccessMode {
        match self {
            Self::FixedValue | Self::MutableValue => ValueAccessMode::Own,
            Self::ReadAlias => ValueAccessMode::Read,
            Self::EditAlias => ValueAccessMode::Update,
        }
    }
    #[must_use]
    pub const fn writable(self) -> bool {
        matches!(self, Self::MutableValue | Self::EditAlias)
    }
}

/// A code declaration, formal input, result family or selected call application.
/// Symbolic origins are static identities, never runtime-erased function pointers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueFunctionOrigin {
    Declaration(crate::AccessCodeId),
    Closure(ValueClosure),
    Parameter(ValueParameter),
    /// A declared callable result, closed by checking its reachable return producers.
    Result(crate::AccessCodeId),
    /// One statically selected call application, independent of source coordinates and type IDs.
    Application(ValueApplication),
}

/// A formal callable input belongs to a header declaration or one lexical anonymous body.
/// Its position is not a runtime field, a source coordinate or a header-arena allocation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueParameter {
    Declaration(crate::AccessParameterId),
    Closure {
        owner: ValueClosure,
        position: usize,
    },
}
impl From<crate::AccessParameterId> for ValueParameter {
    fn from(parameter: crate::AccessParameterId) -> Self {
        Self::Declaration(parameter)
    }
}
impl ValueFunctionOrigin {
    /// Extract a stored code key, without validating its declaration/body domain or availability.
    #[must_use]
    pub const fn code(self) -> Option<ValueCode> {
        match self {
            Self::Declaration(id) => Some(ValueCode::Declaration(id)),
            Self::Closure(id) => Some(ValueCode::Closure(id)),
            Self::Parameter(_) | Self::Result(_) | Self::Application(_) => None,
        }
    }
}

/// A required expression within one complete source body, independent of sibling bodies.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueExpression {
    pub owner: crate::AccessCodeId,
    pub slot: u32,
}

/// A checking/type branch root; local expressions do not enter the declaration arena.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueCodeRoot {
    Declaration(crate::AccessCodeId),
    Expression(ValueExpression),
}
impl From<crate::AccessCodeId> for ValueCodeRoot {
    fn from(id: crate::AccessCodeId) -> Self {
        Self::Declaration(id)
    }
}
impl ValueCodeRoot {
    #[must_use]
    pub const fn source_owner(self) -> crate::AccessCodeId {
        match self {
            Self::Declaration(id) => id,
            Self::Expression(id) => id.owner,
        }
    }
    #[must_use]
    pub const fn code(self) -> ValueCode {
        match self {
            Self::Declaration(id) => ValueCode::Declaration(id),
            Self::Expression(id) => ValueCode::Expression(id),
        }
    }
}

/// Body-owned lexical code identity. It carries no source position or environment address.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueClosure {
    pub owner: ValueCodeRoot,
    pub slot: u32,
}

/// Selected code, independently of a value's invocation view or runtime environment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueCode {
    Declaration(crate::AccessCodeId),
    Expression(ValueExpression),
    Closure(ValueClosure),
}
impl From<crate::AccessCodeId> for ValueCode {
    fn from(id: crate::AccessCodeId) -> Self {
        Self::Declaration(id)
    }
}
impl From<ValueCodeRoot> for ValueCode {
    fn from(root: ValueCodeRoot) -> Self {
        root.code()
    }
}
impl ValueCode {
    #[must_use]
    pub const fn origin(self) -> Option<ValueFunctionOrigin> {
        match self {
            Self::Declaration(id) => Some(ValueFunctionOrigin::Declaration(id)),
            Self::Closure(id) => Some(ValueFunctionOrigin::Closure(id)),
            Self::Expression(_) => None,
        }
    }
    #[must_use]
    pub const fn root(self) -> ValueCodeRoot {
        match self {
            Self::Declaration(id) => ValueCodeRoot::Declaration(id),
            Self::Expression(id) => ValueCodeRoot::Expression(id),
            Self::Closure(id) => id.owner,
        }
    }
    /// The root code whose complete source body owns this code's reuse guard.
    /// This identity does not grant a name, function witness or invocation contract.
    #[must_use]
    pub const fn source_owner(self) -> crate::AccessCodeId {
        self.root().source_owner()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueApplication {
    pub owner: ValueCode,
    pub call: crate::AccessExpressionId,
}

/// Invocation access, independent of a callable value's binding or explicit inputs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueInvocation {
    Read,
    Edit,
    Take,
}
impl ValueInvocation {
    #[must_use]
    pub const fn operation(self) -> ValueInputOperation {
        match self {
            Self::Read => ValueInputOperation::Read,
            Self::Edit => ValueInputOperation::Edit,
            Self::Take => ValueInputOperation::Take,
        }
    }
    #[must_use]
    pub const fn can_supply(self, expected: Self) -> bool {
        matches!(
            (self, expected),
            (Self::Read, _) | (Self::Edit, Self::Edit | Self::Take) | (Self::Take, Self::Take)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccessCodeId, ArenaBuilder, TypeAuthority, TypeKind};
    #[test]
    fn code_projection_does_not_treat_formals_results_or_applications_as_code() {
        let code = ArenaBuilder::<AccessCodeId, ()>::new().insert(());
        let closure = ValueClosure {
            owner: code.into(),
            slot: 0,
        };
        for selected in [ValueCode::Declaration(code), ValueCode::Closure(closure)] {
            assert_eq!(selected.origin().unwrap().code(), Some(selected));
        }
        let parameter = ArenaBuilder::<crate::AccessParameterId, ()>::new().insert(());
        let call = ArenaBuilder::<crate::AccessExpressionId, ()>::new().insert(());
        for symbolic in [
            ValueFunctionOrigin::Parameter(parameter.into()),
            ValueFunctionOrigin::Result(code),
            ValueFunctionOrigin::Application(ValueApplication {
                owner: ValueCode::Declaration(code),
                call,
            }),
        ] {
            assert_eq!(symbolic.code(), None);
        }
    }
    #[test]
    fn named_witness_projection_preserves_identity_and_invocation_view() {
        let mut declarations = ArenaBuilder::<AccessCodeId, ()>::new();
        let first = declarations.insert(());
        let second = declarations.insert(());
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let mut ids = Vec::new();
        for (declaration, invocation) in [
            (first, ValueInvocation::Read),
            (first, ValueInvocation::Edit),
            (second, ValueInvocation::Read),
        ] {
            ids.push(
                types
                    .intern(TypeKind::ValueFunction {
                        origin: ValueFunctionOrigin::Declaration(declaration),
                        invocation,
                    })
                    .unwrap(),
            );
        }
        assert!(ids[0] != ids[1] && ids[0] != ids[2]);
        for id in ids {
            assert_eq!(types.is_concrete(id), Some(true));
            assert!(!types.may_carry_storage(id));
            let projected = types.project(id).unwrap();
            assert_eq!(projected.types().get(projected.root()), types.get(id));
        }
    }
}

#[cfg(test)]
mod formal_tests {
    use super::*;
    use crate::{AccessParameterId, ArenaBuilder, TypeAuthority, TypeKind};
    #[test]
    fn formal_witnesses_preserve_symbolic_storage_and_identity_on_projection() {
        let mut parameters = ArenaBuilder::<AccessParameterId, ()>::new();
        let first = parameters.insert(());
        let second = parameters.insert(());
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let mut ids = Vec::new();
        for parameter in [first, second] {
            let data = types
                .intern(TypeKind::ValueFunction {
                    origin: ValueFunctionOrigin::Parameter(parameter.into()),
                    invocation: ValueInvocation::Read,
                })
                .unwrap();
            assert_eq!(types.is_concrete(data), Some(false));
            assert!(types.may_carry_storage(data));
            let projected = types.project(data).unwrap();
            assert_eq!(projected.types().get(projected.root()), types.get(data));
            ids.push(data);
        }
        assert_ne!(ids[0], ids[1]);
    }
}

#[cfg(test)]
mod result_tests {
    use super::*;
    use crate::{AccessCodeId, ArenaBuilder, TypeAuthority, TypeKind};
    #[test]
    fn result_signature_identity_is_symbolic_until_its_producer_is_closed() {
        let mut declarations = ArenaBuilder::<AccessCodeId, ()>::new();
        let id = declarations.insert(());
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let result = types
            .intern(TypeKind::ValueFunction {
                origin: ValueFunctionOrigin::Result(id),
                invocation: ValueInvocation::Read,
            })
            .unwrap();
        let named = types
            .intern(TypeKind::ValueFunction {
                origin: ValueFunctionOrigin::Declaration(id),
                invocation: ValueInvocation::Read,
            })
            .unwrap();
        assert_ne!(result, named);
        assert_eq!(types.is_concrete(result), Some(false));
        assert!(types.may_carry_storage(result));
        let projected = types.project(result).unwrap();
        assert_eq!(projected.types().get(projected.root()), types.get(result));
    }
}

/// The immutable place owned by one static initializer declaration, distinct from its code role.
/// A declaration program must validate the initializer role before granting a static place.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueStatic {
    pub initializer: crate::AccessCodeId,
}
