use std::collections::HashMap;

use nocter_model::{
    BorrowCapability, CallableCapability, CallableContract, ClosureId, GenericParameterId,
    InterfaceId, InvalidParameterOrigin, NominalTypeId, OpaqueTypeId, ResultProvenance, TypeId,
    TypeKind, TypeStore, TypeTransaction, UnknownTypeId,
};

/// Reference from one body-local type extension to either its immutable program prefix or an
/// earlier type in the same extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyTypeRef(BodyTypeReference);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BodyTypeReference {
    Program(TypeId),
    Local(u32),
}

impl BodyTypeRef {
    const fn program(ty: TypeId) -> Self {
        Self(BodyTypeReference::Program(ty))
    }

    const fn local(index: u32) -> Self {
        Self(BodyTypeReference::Local(index))
    }
}

/// Body-local closure identity retained inside a reusable type extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyClosureRef(u32);

impl BodyClosureRef {
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BodyTypeKind {
    GenericParameter(GenericParameterId),
    InterfaceSelf(InterfaceId),
    Nominal {
        definition: NominalTypeId,
        arguments: Box<[BodyTypeRef]>,
    },
    AssociatedProjection {
        base: BodyTypeRef,
        associated: nocter_model::AssociatedTypeId,
    },
    Opaque {
        definition: OpaqueTypeId,
        arguments: Box<[BodyTypeRef]>,
    },
    Pointer(BodyTypeRef),
    Borrow {
        capability: BorrowCapability,
        referent: BodyTypeRef,
    },
    Slice(BodyTypeRef),
    FixedArray {
        element: BodyTypeRef,
        length: u64,
    },
    Closure {
        definition: BodyClosureRef,
        arguments: Box<[BodyTypeRef]>,
    },
    Callable {
        capability: CallableCapability,
        parameters: Box<[BodyTypeRef]>,
        pack: Option<BodyTypeRef>,
        result: BodyTypeRef,
        provenance: ResultProvenance,
    },
    Optional(BodyTypeRef),
    Fallible(BodyTypeRef),
}

/// Source-neutral structural types added while checking one body.
///
/// Program type identities remain references into the exact reusable preparation authority.
/// Types and closures created by this body use dense local identities, so a preceding body's
/// additions cannot invalidate this recipe. Replay interns the extension into the current
/// canonical program branch and returns the local-to-current identity map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyTypeRecipe {
    program_type_count: usize,
    additions: Box<[BodyTypeKind]>,
}

impl BodyTypeRecipe {
    /// Captures only the suffix added to `program` by one body transaction.
    ///
    /// `body_closures` must map every closure referenced by that suffix to its body-local identity.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when `branch` does not preserve the exact program prefix, a type
    /// references neither that prefix nor an earlier suffix entry, or a closure is not owned by
    /// this body.
    pub fn capture(
        program: &TypeStore,
        branch: &TypeStore,
        body_closures: &HashMap<ClosureId, BodyClosureRef>,
    ) -> Result<Self, BodyTypeRecipeError> {
        if branch.type_count() < program.type_count()
            || program
                .iter()
                .any(|(ty, kind)| branch.get(ty) != Some(kind))
        {
            return Err(BodyTypeRecipeError::ProgramPrefixMismatch);
        }
        let local_ids = branch
            .iter_from(program.type_count())
            .enumerate()
            .map(|(index, (ty, _))| {
                u32::try_from(index)
                    .map(|index| (ty, BodyTypeRef::local(index)))
                    .map_err(|_| BodyTypeRecipeError::TooManyLocalTypes)
            })
            .collect::<Result<HashMap<_, _>, _>>()?;
        let reference = |ty| {
            if program.get(ty).is_some() {
                Ok(BodyTypeRef::program(ty))
            } else {
                local_ids
                    .get(&ty)
                    .copied()
                    .ok_or(BodyTypeRecipeError::UnknownType(ty))
            }
        };
        let additions = branch
            .iter_from(program.type_count())
            .map(|(_, kind)| capture_kind(kind, &reference, body_closures))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            program_type_count: program.type_count(),
            additions: additions.into_boxed_slice(),
        })
    }

    /// Replays this body-local extension into a current canonical type branch.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when the program prefix differs, a local reference points
    /// forward, the closure map is incomplete, or the reconstructed structural type is invalid.
    pub fn replay(
        &self,
        program: &TypeStore,
        target: &mut TypeTransaction,
        closures: &[ClosureId],
    ) -> Result<ReplayedBodyTypes, BodyTypeRecipeError> {
        if program.type_count() != self.program_type_count
            || program
                .iter()
                .any(|(ty, kind)| target.get(ty) != Some(kind))
        {
            return Err(BodyTypeRecipeError::ProgramPrefixMismatch);
        }
        let mut locals = Vec::with_capacity(self.additions.len());
        for kind in &self.additions {
            let kind = replay_kind(kind, &locals, closures)?;
            locals.push(
                target
                    .intern(kind)
                    .map_err(BodyTypeRecipeError::InvalidType)?,
            );
        }
        Ok(ReplayedBodyTypes {
            locals: locals.into_boxed_slice(),
        })
    }
}

/// Current identities assigned while replaying one [`BodyTypeRecipe`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayedBodyTypes {
    locals: Box<[TypeId]>,
}

impl ReplayedBodyTypes {
    /// Resolves one recipe reference in the current canonical type branch.
    ///
    /// # Errors
    ///
    /// Returns an integrity error for an unknown local identity. Program references were validated
    /// when their recipe was captured and are returned unchanged.
    pub fn resolve(&self, reference: BodyTypeRef) -> Result<TypeId, BodyTypeRecipeError> {
        match reference.0 {
            BodyTypeReference::Program(ty) => Ok(ty),
            BodyTypeReference::Local(index) => self
                .locals
                .get(index as usize)
                .copied()
                .ok_or(BodyTypeRecipeError::UnknownLocalType(index)),
        }
    }

    #[must_use]
    pub const fn locals(&self) -> &[TypeId] {
        &self.locals
    }
}

fn capture_kind(
    kind: &TypeKind,
    reference: &impl Fn(TypeId) -> Result<BodyTypeRef, BodyTypeRecipeError>,
    body_closures: &HashMap<ClosureId, BodyClosureRef>,
) -> Result<BodyTypeKind, BodyTypeRecipeError> {
    Ok(match kind {
        TypeKind::Builtin(_) => return Err(BodyTypeRecipeError::BuiltinInExtension),
        TypeKind::GenericParameter(parameter) => BodyTypeKind::GenericParameter(*parameter),
        TypeKind::InterfaceSelf(interface) => BodyTypeKind::InterfaceSelf(*interface),
        TypeKind::Nominal {
            definition,
            arguments,
        } => BodyTypeKind::Nominal {
            definition: *definition,
            arguments: capture_types(arguments, reference)?,
        },
        TypeKind::AssociatedProjection { base, associated } => BodyTypeKind::AssociatedProjection {
            base: reference(*base)?,
            associated: *associated,
        },
        TypeKind::Opaque {
            definition,
            arguments,
        } => BodyTypeKind::Opaque {
            definition: *definition,
            arguments: capture_types(arguments, reference)?,
        },
        TypeKind::Pointer(pointee) => BodyTypeKind::Pointer(reference(*pointee)?),
        TypeKind::Borrow {
            capability,
            referent,
        } => BodyTypeKind::Borrow {
            capability: *capability,
            referent: reference(*referent)?,
        },
        TypeKind::Slice(element) => BodyTypeKind::Slice(reference(*element)?),
        TypeKind::FixedArray { element, length } => BodyTypeKind::FixedArray {
            element: reference(*element)?,
            length: *length,
        },
        TypeKind::Closure {
            definition,
            arguments,
        } => BodyTypeKind::Closure {
            definition: body_closures
                .get(definition)
                .copied()
                .ok_or(BodyTypeRecipeError::UnknownClosure(*definition))?,
            arguments: capture_types(arguments, reference)?,
        },
        TypeKind::Callable(contract) => BodyTypeKind::Callable {
            capability: contract.capability(),
            parameters: capture_types(contract.parameters(), reference)?,
            pack: contract.pack().map(reference).transpose()?,
            result: reference(contract.result())?,
            provenance: contract.provenance().clone(),
        },
        TypeKind::Optional(payload) => BodyTypeKind::Optional(reference(*payload)?),
        TypeKind::Fallible(payload) => BodyTypeKind::Fallible(reference(*payload)?),
    })
}

fn capture_types(
    types: &[TypeId],
    reference: &impl Fn(TypeId) -> Result<BodyTypeRef, BodyTypeRecipeError>,
) -> Result<Box<[BodyTypeRef]>, BodyTypeRecipeError> {
    types
        .iter()
        .copied()
        .map(reference)
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn replay_kind(
    kind: &BodyTypeKind,
    locals: &[TypeId],
    closures: &[ClosureId],
) -> Result<TypeKind, BodyTypeRecipeError> {
    let resolve = |reference: BodyTypeRef| match reference.0 {
        BodyTypeReference::Program(ty) => Ok(ty),
        BodyTypeReference::Local(index) => locals
            .get(index as usize)
            .copied()
            .ok_or(BodyTypeRecipeError::ForwardLocalType(index)),
    };
    Ok(match kind {
        BodyTypeKind::GenericParameter(parameter) => TypeKind::GenericParameter(*parameter),
        BodyTypeKind::InterfaceSelf(interface) => TypeKind::InterfaceSelf(*interface),
        BodyTypeKind::Nominal {
            definition,
            arguments,
        } => TypeKind::Nominal {
            definition: *definition,
            arguments: replay_types(arguments, &resolve)?,
        },
        BodyTypeKind::AssociatedProjection { base, associated } => TypeKind::AssociatedProjection {
            base: resolve(*base)?,
            associated: *associated,
        },
        BodyTypeKind::Opaque {
            definition,
            arguments,
        } => TypeKind::Opaque {
            definition: *definition,
            arguments: replay_types(arguments, &resolve)?,
        },
        BodyTypeKind::Pointer(pointee) => TypeKind::Pointer(resolve(*pointee)?),
        BodyTypeKind::Borrow {
            capability,
            referent,
        } => TypeKind::Borrow {
            capability: *capability,
            referent: resolve(*referent)?,
        },
        BodyTypeKind::Slice(element) => TypeKind::Slice(resolve(*element)?),
        BodyTypeKind::FixedArray { element, length } => TypeKind::FixedArray {
            element: resolve(*element)?,
            length: *length,
        },
        BodyTypeKind::Closure {
            definition,
            arguments,
        } => TypeKind::Closure {
            definition: closures
                .get(definition.index() as usize)
                .copied()
                .ok_or(BodyTypeRecipeError::UnknownLocalClosure(definition.index()))?,
            arguments: replay_types(arguments, &resolve)?,
        },
        BodyTypeKind::Callable {
            capability,
            parameters,
            pack,
            result,
            provenance,
        } => TypeKind::Callable(
            CallableContract::new(
                *capability,
                replay_types(parameters, &resolve)?,
                pack.map(resolve).transpose()?,
                resolve(*result)?,
                provenance.clone(),
            )
            .map_err(BodyTypeRecipeError::InvalidCallable)?,
        ),
        BodyTypeKind::Optional(payload) => TypeKind::Optional(resolve(*payload)?),
        BodyTypeKind::Fallible(payload) => TypeKind::Fallible(resolve(*payload)?),
    })
}

fn replay_types(
    types: &[BodyTypeRef],
    resolve: &impl Fn(BodyTypeRef) -> Result<TypeId, BodyTypeRecipeError>,
) -> Result<Box<[TypeId]>, BodyTypeRecipeError> {
    types
        .iter()
        .copied()
        .map(resolve)
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyTypeRecipeError {
    ProgramPrefixMismatch,
    TooManyLocalTypes,
    BuiltinInExtension,
    UnknownType(TypeId),
    UnknownLocalType(u32),
    ForwardLocalType(u32),
    UnknownClosure(ClosureId),
    UnknownLocalClosure(u32),
    InvalidType(UnknownTypeId),
    InvalidCallable(InvalidParameterOrigin),
}

impl std::fmt::Display for BodyTypeRecipeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid body type recipe: {self:?}")
    }
}

impl std::error::Error for BodyTypeRecipeError {}

#[cfg(test)]
mod tests {
    use super::{BodyClosureRef, BodyTypeRecipe};
    use nocter_model::{BuiltinType, ClosureSequence, TypeAuthority, TypeKind};
    use std::collections::HashMap;

    #[test]
    fn replay_is_independent_of_types_added_by_a_preceding_body() {
        let program = TypeAuthority::new();
        let i32_ = program.store().builtin(BuiltinType::I32);
        let mut source = program.transaction();
        let optional = source.intern(TypeKind::Optional(i32_)).unwrap();
        let fallible = source.intern(TypeKind::Fallible(optional)).unwrap();
        let recipe = BodyTypeRecipe::capture(program.store(), &source, &HashMap::new()).unwrap();

        let mut target = program.transaction();
        let _preceding = target.intern(TypeKind::Pointer(i32_)).unwrap();
        let replayed = recipe.replay(program.store(), &mut target, &[]).unwrap();

        assert_ne!(replayed.locals()[0], optional);
        assert!(matches!(
            target.get(replayed.locals()[1]),
            Some(TypeKind::Fallible(payload)) if *payload == replayed.locals()[0]
        ));
        assert_ne!(replayed.locals()[1], fallible);
    }

    #[test]
    fn closure_identity_is_rebound_separately_from_structural_types() {
        let program = TypeAuthority::new();
        let mut local_closures = ClosureSequence::default();
        let local = local_closures.insert(());
        let mut source = program.transaction();
        let _closure_type = source
            .intern(TypeKind::Closure {
                definition: local,
                arguments: Box::new([]),
            })
            .unwrap();
        let recipe = BodyTypeRecipe::capture(
            program.store(),
            &source,
            &HashMap::from([(local, BodyClosureRef::new(0))]),
        )
        .unwrap();

        let mut global_closures = ClosureSequence::default();
        let _preceding = global_closures.insert(());
        let current = global_closures.insert(());
        let mut target = program.transaction();
        let replayed = recipe
            .replay(program.store(), &mut target, &[current])
            .unwrap();

        assert!(matches!(
            target.get(replayed.locals()[0]),
            Some(TypeKind::Closure { definition, .. }) if *definition == current
        ));
    }
}
