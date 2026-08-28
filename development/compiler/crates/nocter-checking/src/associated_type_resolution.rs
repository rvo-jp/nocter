use std::collections::{HashMap, HashSet};
use std::fmt;

use nocter_declarations::DeclarationGraph;
use nocter_model::{AssociatedTypeId, TypeId, TypeKind};

use crate::interface_implementation::{
    AssociatedImplementationSelection, InterfaceImplementationTable, RequirementPredicate,
    resolve_selected_associated_type, select_associated_implementation,
};
use crate::type_relations::{SubstitutionError, map_type_children};
use crate::{CheckedPredicate, is_concrete_type};

/// The sole semantic authority for reducing associated projections after their names are bound.
///
/// A symbolic base remains a projection. Once its base is concrete, the exact checked interface
/// implementation selects and substitutes the associated value. Both body checking and executable
/// specialization consume this contract.
pub(crate) struct AssociatedTypeResolver<'program, R> {
    graph: &'program DeclarationGraph,
    implementations: &'program InterfaceImplementationTable,
    assumptions: &'program [R],
    intrinsic_facts: &'program [CheckedPredicate],
}

impl<'program, R: RequirementPredicate> AssociatedTypeResolver<'program, R> {
    pub(crate) const fn new(
        graph: &'program DeclarationGraph,
        implementations: &'program InterfaceImplementationTable,
        assumptions: &'program [R],
        intrinsic_facts: &'program [CheckedPredicate],
    ) -> Self {
        Self {
            graph,
            implementations,
            assumptions,
            intrinsic_facts,
        }
    }

    pub(crate) fn reduce(
        &self,
        types: &mut nocter_model::TypeTransaction,
        root: TypeId,
    ) -> Result<TypeId, AssociatedTypeResolutionError> {
        self.reduce_type(types, root, &mut HashMap::new(), &mut HashSet::new())
    }

    fn reduce_type(
        &self,
        types: &mut nocter_model::TypeTransaction,
        ty: TypeId,
        finished: &mut HashMap<TypeId, TypeId>,
        active: &mut HashSet<TypeId>,
    ) -> Result<TypeId, AssociatedTypeResolutionError> {
        if let Some(reduced) = finished.get(&ty) {
            return Ok(*reduced);
        }
        if !active.insert(ty) {
            return Err(AssociatedTypeResolutionError::RecursiveProjection(ty));
        }
        let kind = types
            .get(ty)
            .cloned()
            .ok_or(AssociatedTypeResolutionError::UnknownType(ty))?;
        let reduced = if let TypeKind::AssociatedProjection { base, associated } = kind {
            let base = self.reduce_type(types, base, finished, active)?;
            if is_concrete_type(types, base)? {
                let declaration = self
                    .graph
                    .declarations()
                    .associated_types()
                    .get(associated)
                    .ok_or(AssociatedTypeResolutionError::MissingAssociatedType(
                        associated,
                    ))?;
                let selection = select_associated_implementation(
                    types,
                    self.implementations,
                    self.assumptions,
                    self.intrinsic_facts,
                    base,
                    declaration.interface(),
                )?;
                let selected = match selection {
                    AssociatedImplementationSelection::None => {
                        return Err(AssociatedTypeResolutionError::UnavailableImplementation {
                            base,
                            associated,
                        });
                    }
                    AssociatedImplementationSelection::Ambiguous => {
                        return Err(AssociatedTypeResolutionError::AmbiguousImplementation {
                            base,
                            associated,
                        });
                    }
                    AssociatedImplementationSelection::Unique(selection) => selection,
                };
                let value = resolve_selected_associated_type(
                    types,
                    self.implementations,
                    &selected,
                    associated,
                )?;
                self.reduce_type(types, value, finished, active)?
            } else {
                types
                    .intern(TypeKind::AssociatedProjection { base, associated })
                    .map_err(|unknown| AssociatedTypeResolutionError::UnknownType(unknown.id()))?
            }
        } else {
            let rebuilt = map_type_children(kind, |child| {
                self.reduce_type(types, child, finished, active)
            })?;
            types
                .intern(rebuilt)
                .map_err(|unknown| AssociatedTypeResolutionError::UnknownType(unknown.id()))?
        };
        active.remove(&ty);
        finished.insert(ty, reduced);
        Ok(reduced)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssociatedTypeResolutionError {
    UnknownType(TypeId),
    MissingAssociatedType(AssociatedTypeId),
    UnavailableImplementation {
        base: TypeId,
        associated: AssociatedTypeId,
    },
    AmbiguousImplementation {
        base: TypeId,
        associated: AssociatedTypeId,
    },
    RecursiveProjection(TypeId),
    Substitution(SubstitutionError),
}

impl fmt::Display for AssociatedTypeResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "associated type resolution failed: {self:?}")
    }
}

impl std::error::Error for AssociatedTypeResolutionError {}

impl From<SubstitutionError> for AssociatedTypeResolutionError {
    fn from(error: SubstitutionError) -> Self {
        Self::Substitution(error)
    }
}
