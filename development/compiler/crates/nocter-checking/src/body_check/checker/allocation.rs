use nocter_model::{BodyNodeId, BorrowCapability, TypeId, TypeKind};
use nocter_syntax::{NodeId, NodeKind};
use nocter_toolchain_contract::StandardDeclarationRole;

use super::{BodyChecker, ResolvedPlace};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::direct_node;
use crate::{CheckedOperation, PlaceAccess};

impl BodyChecker<'_, '_> {
    pub(super) fn check_allocation_place(
        &mut self,
        allocator: NodeId,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let (named, place) = self.resolve_allocation_place(allocator)?;
        self.add_node(named, place.ty, CheckedOperation::Place(place.id))
    }

    /// Retains a region parent for the complete child lifetime.
    ///
    /// An existing borrow value already carries its external loan. An owned allocator/context is
    /// borrowed readonly so moving, dropping, or mutating the parent while the child exists is
    /// rejected by the ordinary loan analysis.
    pub(super) fn check_region_parent(
        &mut self,
        allocator: NodeId,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let (named, place) = self.resolve_allocation_place(allocator)?;
        if matches!(self.types.get(place.ty), Some(TypeKind::Borrow { .. })) {
            return self.add_node(named, place.ty, CheckedOperation::Place(place.id));
        }
        if place.access != PlaceAccess::Owned {
            return Err(self.rule(BodyRule::InvalidAllocationContext, named)?);
        }
        let ty = self
            .types
            .intern(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent: place.ty,
            })
            .map_err(|_| BodyCheckInternalError::UnknownType(place.ty))?;
        self.add_node(
            named,
            ty,
            CheckedOperation::Borrow {
                capability: BorrowCapability::Readonly,
                place: place.id,
            },
        )
    }

    pub(super) fn allocation_context_type(&mut self) -> Result<TypeId, BodyCheckInternalError> {
        let definition = self
            .standard_semantics
            .nominal(StandardDeclarationRole::AllocationContext)
            .ok_or(BodyCheckInternalError::MissingAllocationSemanticRoles)?;
        self.types
            .intern(TypeKind::Nominal {
                definition,
                arguments: Box::new([]),
            })
            .map_err(|_| BodyCheckInternalError::MissingAllocationSemanticRoles)
    }

    fn resolve_allocation_place(
        &mut self,
        allocator: NodeId,
    ) -> Result<(NodeId, ResolvedPlace), BodyCheckError> {
        let named = direct_node(self.tree(), allocator, NodeKind::NamedPlace)
            .ok_or(BodyCheckInternalError::InvalidSyntax(allocator))?;
        let place = self.named_place(named)?;
        let candidate = match self.types.get(place.ty) {
            Some(TypeKind::Borrow { referent, .. }) => *referent,
            Some(_) => place.ty,
            None => return Err(BodyCheckInternalError::UnknownType(place.ty).into()),
        };
        let Some(TypeKind::Nominal {
            definition,
            arguments,
        }) = self.types.get(candidate)
        else {
            return Err(self.rule(BodyRule::InvalidAllocationContext, named)?);
        };
        let roles = [
            self.standard_semantics
                .nominal(StandardDeclarationRole::AbortingAllocator),
            self.standard_semantics
                .nominal(StandardDeclarationRole::AllocationContext),
        ];
        if roles.iter().all(Option::is_none) {
            return Err(BodyCheckInternalError::MissingAllocationSemanticRoles.into());
        }
        if !arguments.is_empty() || !roles.into_iter().flatten().any(|role| role == *definition) {
            return Err(self.rule(BodyRule::InvalidAllocationContext, named)?);
        }
        Ok((named, place))
    }
}
