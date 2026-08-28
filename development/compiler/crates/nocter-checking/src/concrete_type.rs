use nocter_model::{TypeId, TypeKind};

use crate::associated_type_resolution::{AssociatedTypeResolutionError, AssociatedTypeResolver};
use crate::concrete_dispatch::ConcreteDispatchResolver;
use crate::type_relations::TypeSubstitution;
use crate::{ConcreteDestructionError, is_concrete_type};

impl ConcreteDispatchResolver<'_> {
    /// Interns one newly assembled concrete type in the specialization store.
    ///
    /// # Errors
    ///
    /// Rejects an unknown referenced type or a kind that still contains symbolic components.
    pub fn intern_concrete(&mut self, kind: TypeKind) -> Result<TypeId, ConcreteDestructionError> {
        let ty = self
            .types_mut()
            .intern(kind)
            .map_err(|unknown| ConcreteDestructionError::UnknownType(unknown.id()))?;
        if !is_concrete_type(self.types(), ty)? {
            return Err(ConcreteDestructionError::SymbolicType(ty));
        }
        Ok(ty)
    }

    /// Applies one enclosing specialization and recursively resolves every associated projection.
    ///
    /// Generic substitution alone cannot reduce `T.Item`: after `T` becomes concrete, the selected
    /// interface implementation owns the value of `Item`. Body checking and executable freezing
    /// consume the same associated-type resolver for this second step.
    ///
    /// # Errors
    ///
    /// Returns a typed invariant failure when substitution is incomplete, associated evidence is
    /// unavailable or ambiguous, or the resulting type remains symbolic.
    pub fn specialize_type(
        &mut self,
        ty: TypeId,
        enclosing: &TypeSubstitution,
    ) -> Result<TypeId, ConcreteDestructionError> {
        let substituted = enclosing.apply_type(self.types_mut(), ty)?;
        let program = self.program;
        let resolver = AssociatedTypeResolver::new(
            program.graph(),
            program.interface_implementations(),
            &[] as &[crate::CheckedRequirement],
            &[],
        );
        let reduced = resolver.reduce(self.types_mut(), substituted)?;
        if !is_concrete_type(self.types(), reduced)? {
            return Err(ConcreteDestructionError::SymbolicType(reduced));
        }
        Ok(reduced)
    }
}

impl From<AssociatedTypeResolutionError> for ConcreteDestructionError {
    fn from(error: AssociatedTypeResolutionError) -> Self {
        match error {
            AssociatedTypeResolutionError::UnknownType(ty) => Self::UnknownType(ty),
            AssociatedTypeResolutionError::MissingAssociatedType(associated) => {
                Self::MissingAssociatedType(associated)
            }
            AssociatedTypeResolutionError::UnavailableImplementation { base, associated } => {
                Self::UnavailableAssociatedImplementation { base, associated }
            }
            AssociatedTypeResolutionError::AmbiguousImplementation { base, associated } => {
                Self::AmbiguousAssociatedImplementation { base, associated }
            }
            AssociatedTypeResolutionError::RecursiveProjection(ty) => {
                Self::RecursiveAssociatedProjection(ty)
            }
            AssociatedTypeResolutionError::Substitution(error) => Self::Substitution(error),
        }
    }
}
