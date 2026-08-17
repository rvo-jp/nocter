use std::collections::BTreeMap;
use std::fmt;
use std::ops::{Deref, DerefMut};

use nocter_model::{
    ArenaBuilder, ExecutableItemId, MirBlockId, MirDropFlagId, MirLocalId, MirOperationId,
    MirPlaceId, MirValueId, TypeId,
};

use crate::schema::MirBodyDomains;
use crate::{
    MirBlock, MirBody, MirDropFlag, MirFunction, MirLocal, MirLocalKind, MirOperation,
    MirOperationKind, MirPackInput, MirPlace, MirPlaceRoot, MirTerminator,
    MirValidationEnvironment, MirValidationError, MirValue, MirValueDefinition, validate_function,
};

/// Mutable state for one basic block while a function is being built.
#[derive(Debug, Default)]
pub struct MirBlockBuilder {
    parameters: Vec<MirValueId>,
    operations: Vec<MirOperationId>,
    terminator: Option<MirTerminator>,
}

impl MirBlockBuilder {
    #[must_use]
    pub const fn parameters(&self) -> &[MirValueId] {
        self.parameters.as_slice()
    }

    #[must_use]
    pub const fn operations(&self) -> &[MirOperationId] {
        self.operations.as_slice()
    }

    #[must_use]
    pub const fn terminator(&self) -> Option<&MirTerminator> {
        self.terminator.as_ref()
    }
}

/// The sole mutable construction path for the CFG domains shared by functions and roots.
#[derive(Debug)]
pub struct MirBodyBuilder {
    parameters: Vec<MirLocalId>,
    pack: Option<MirPackInput>,
    locals: ArenaBuilder<MirLocalId, MirLocal>,
    drop_flags: ArenaBuilder<MirDropFlagId, MirDropFlag>,
    places: ArenaBuilder<MirPlaceId, MirPlace>,
    places_by_shape: BTreeMap<MirPlace, MirPlaceId>,
    values: ArenaBuilder<MirValueId, MirValue>,
    operations: ArenaBuilder<MirOperationId, MirOperation>,
    blocks: ArenaBuilder<MirBlockId, MirBlockBuilder>,
}

impl Default for MirBodyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MirBodyBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            parameters: Vec::new(),
            pack: None,
            locals: ArenaBuilder::new(),
            drop_flags: ArenaBuilder::new(),
            places: ArenaBuilder::new(),
            places_by_shape: BTreeMap::new(),
            values: ArenaBuilder::new(),
            operations: ArenaBuilder::new(),
            blocks: ArenaBuilder::new(),
        }
    }

    pub fn add_parameter(&mut self, ty: TypeId, mutable: bool) -> MirLocalId {
        let position = self.parameters.len();
        let local = self.locals.insert(MirLocal::new(
            ty,
            MirLocalKind::Parameter { position },
            mutable,
        ));
        self.parameters.push(local);
        local
    }

    /// Installs the one non-ABI sequence pack input accepted by a literal body.
    ///
    /// # Errors
    ///
    /// Returns [`MirBodyBuildError::DuplicatePackInput`] when the input was already installed.
    pub fn set_pack_input(&mut self, input: MirPackInput) -> Result<(), MirBodyBuildError> {
        if self.pack.replace(input).is_some() {
            return Err(MirBodyBuildError::DuplicatePackInput);
        }
        Ok(())
    }

    pub fn add_local(&mut self, ty: TypeId, kind: MirLocalKind, mutable: bool) -> MirLocalId {
        self.locals.insert(MirLocal::new(ty, kind, mutable))
    }

    #[must_use]
    pub fn local_type(&self, local: MirLocalId) -> Option<TypeId> {
        self.locals.get(local).copied().map(MirLocal::ty)
    }

    #[must_use]
    pub fn value_type(&self, value: MirValueId) -> Option<TypeId> {
        self.values.get(value).copied().map(MirValue::ty)
    }

    pub fn add_place(
        &mut self,
        root: MirPlaceRoot,
        projections: impl Into<Box<[crate::MirProjection]>>,
        ty: TypeId,
    ) -> MirPlaceId {
        let place = MirPlace::new(root, projections, ty);
        if let Some(existing) = self.places_by_shape.get(&place).copied() {
            return existing;
        }
        let id = self.places.insert(place.clone());
        self.places_by_shape.insert(place, id);
        id
    }

    #[must_use]
    pub fn place(&self, place: MirPlaceId) -> Option<&MirPlace> {
        self.places.get(place)
    }

    pub fn add_drop_flag(
        &mut self,
        place: MirPlaceId,
        initially_initialized: bool,
    ) -> MirDropFlagId {
        self.drop_flags
            .insert(MirDropFlag::new(place, initially_initialized))
    }

    /// Creates a block and its typed SSA parameters as one identity-safe operation.
    pub fn create_block(
        &mut self,
        parameter_types: impl IntoIterator<Item = TypeId>,
    ) -> (MirBlockId, Box<[MirValueId]>) {
        let block = self.blocks.next_id();
        let parameters = parameter_types
            .into_iter()
            .enumerate()
            .map(|(position, ty)| {
                self.values.insert(MirValue::new(
                    ty,
                    MirValueDefinition::BlockParameter { block, position },
                ))
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let inserted = self.blocks.insert(MirBlockBuilder {
            parameters: parameters.to_vec(),
            operations: Vec::new(),
            terminator: None,
        });
        debug_assert_eq!(inserted, block);
        (block, parameters)
    }

    /// Appends a value-producing instruction in exact execution order.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or already terminated block, or an effect-only operation kind.
    pub fn append_value(
        &mut self,
        block: MirBlockId,
        ty: TypeId,
        kind: MirOperationKind,
    ) -> Result<MirValueId, MirBodyBuildError> {
        if !kind.produces_value() {
            return Err(MirBodyBuildError::EffectKindUsedAsValue);
        }
        let block_data = self
            .blocks
            .get_mut(block)
            .ok_or(MirBodyBuildError::UnknownBlock(block))?;
        if block_data.terminator.is_some() {
            return Err(MirBodyBuildError::AlreadyTerminated);
        }
        let operation = self.operations.next_id();
        let value = self
            .values
            .insert(MirValue::new(ty, MirValueDefinition::Operation(operation)));
        let inserted = self.operations.insert(MirOperation::new(kind, Some(value)));
        debug_assert_eq!(inserted, operation);
        block_data.operations.push(operation);
        Ok(value)
    }

    /// Appends an effect-only instruction in exact execution order.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or already terminated block, or a value-producing operation kind.
    pub fn append_effect(
        &mut self,
        block: MirBlockId,
        kind: MirOperationKind,
    ) -> Result<MirOperationId, MirBodyBuildError> {
        if kind.produces_value() {
            return Err(MirBodyBuildError::ValueKindUsedAsEffect);
        }
        let block_data = self
            .blocks
            .get_mut(block)
            .ok_or(MirBodyBuildError::UnknownBlock(block))?;
        if block_data.terminator.is_some() {
            return Err(MirBodyBuildError::AlreadyTerminated);
        }
        let operation = self.operations.insert(MirOperation::new(kind, None));
        block_data.operations.push(operation);
        Ok(operation)
    }

    /// Closes one block exactly once.
    ///
    /// # Errors
    ///
    /// Rejects an unknown or already terminated block.
    pub fn terminate(
        &mut self,
        block: MirBlockId,
        terminator: MirTerminator,
    ) -> Result<(), MirBodyBuildError> {
        let block = self
            .blocks
            .get_mut(block)
            .ok_or(MirBodyBuildError::UnknownBlock(block))?;
        if block.terminator.is_some() {
            return Err(MirBodyBuildError::AlreadyTerminated);
        }
        block.terminator = Some(terminator);
        Ok(())
    }

    /// Freezes the complete body. The owning function or root performs contract validation.
    ///
    /// # Errors
    ///
    /// Returns an error for an unterminated block.
    pub fn finish(self, entry: MirBlockId) -> Result<MirBody, MirBodyBuildError> {
        let blocks = self.blocks.try_finish_with(|block, draft| {
            let terminator = draft
                .terminator
                .ok_or(MirBodyBuildError::UnterminatedBlock(block))?;
            Ok::<MirBlock, MirBodyBuildError>(MirBlock::new(
                draft.parameters,
                draft.operations,
                terminator,
            ))
        })?;
        Ok(MirBody::new(
            self.parameters,
            self.pack,
            MirBodyDomains {
                locals: self.locals.finish(),
                drop_flags: self.drop_flags.finish(),
                places: self.places.finish(),
                values: self.values.finish(),
                operations: self.operations.finish(),
                blocks,
            },
            entry,
        ))
    }
}

#[derive(Debug)]
pub enum MirBodyBuildError {
    UnknownBlock(MirBlockId),
    AlreadyTerminated,
    UnterminatedBlock(MirBlockId),
    EffectKindUsedAsValue,
    ValueKindUsedAsEffect,
    DuplicatePackInput,
    Validation(MirValidationError),
}

impl fmt::Display for MirBodyBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MIR body construction failed: {self:?}")
    }
}

impl std::error::Error for MirBodyBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            Self::UnknownBlock(_)
            | Self::AlreadyTerminated
            | Self::UnterminatedBlock(_)
            | Self::EffectKindUsedAsValue
            | Self::ValueKindUsedAsEffect
            | Self::DuplicatePackInput => None,
        }
    }
}

impl From<MirValidationError> for MirBodyBuildError {
    fn from(error: MirValidationError) -> Self {
        Self::Validation(error)
    }
}

/// Mutable construction of one callable and its shared MIR body.
#[derive(Debug)]
pub struct MirFunctionBuilder {
    item: ExecutableItemId,
    result: TypeId,
    body: MirBodyBuilder,
}

impl MirFunctionBuilder {
    #[must_use]
    pub fn new(item: ExecutableItemId, result: TypeId) -> Self {
        Self {
            item,
            result,
            body: MirBodyBuilder::new(),
        }
    }

    /// Freezes and validates the complete function.
    ///
    /// # Errors
    ///
    /// Returns an error for an unterminated block or any closed MIR invariant violation.
    pub fn finish(
        self,
        entry: MirBlockId,
        environment: &impl MirValidationEnvironment,
    ) -> Result<MirFunction, MirBodyBuildError> {
        let function = MirFunction::new(self.item, self.result, self.body.finish(entry)?);
        validate_function(&function, environment)?;
        Ok(function)
    }
}

impl Deref for MirFunctionBuilder {
    type Target = MirBodyBuilder;

    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl DerefMut for MirFunctionBuilder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.body
    }
}
