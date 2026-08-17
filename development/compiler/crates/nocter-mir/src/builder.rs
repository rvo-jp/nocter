use std::fmt;

use nocter_model::{
    ArenaBuilder, ExecutableItemId, MirBlockId, MirDropFlagId, MirLocalId, MirOperationId,
    MirPlaceId, MirValueId, TypeId,
};

use crate::schema::MirFunctionDomains;
use crate::{
    MirBlock, MirDropFlag, MirFunction, MirLocal, MirLocalKind, MirOperation, MirOperationKind,
    MirPlace, MirPlaceRoot, MirTerminator, MirValidationEnvironment, MirValidationError, MirValue,
    MirValueDefinition, validate_function,
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

/// The sole mutable construction path for a [`MirFunction`].
#[derive(Debug)]
pub struct MirFunctionBuilder {
    item: ExecutableItemId,
    result: TypeId,
    parameters: Vec<MirLocalId>,
    locals: ArenaBuilder<MirLocalId, MirLocal>,
    drop_flags: ArenaBuilder<MirDropFlagId, MirDropFlag>,
    places: ArenaBuilder<MirPlaceId, MirPlace>,
    values: ArenaBuilder<MirValueId, MirValue>,
    operations: ArenaBuilder<MirOperationId, MirOperation>,
    blocks: ArenaBuilder<MirBlockId, MirBlockBuilder>,
}

impl MirFunctionBuilder {
    #[must_use]
    pub fn new(item: ExecutableItemId, result: TypeId) -> Self {
        Self {
            item,
            result,
            parameters: Vec::new(),
            locals: ArenaBuilder::new(),
            drop_flags: ArenaBuilder::new(),
            places: ArenaBuilder::new(),
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

    pub fn add_local(&mut self, ty: TypeId, kind: MirLocalKind, mutable: bool) -> MirLocalId {
        self.locals.insert(MirLocal::new(ty, kind, mutable))
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
        self.places.insert(MirPlace::new(root, projections, ty))
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
    ) -> Result<MirValueId, MirFunctionBuildError> {
        if !kind.produces_value() {
            return Err(MirFunctionBuildError::EffectKindUsedAsValue);
        }
        let block_data = self
            .blocks
            .get_mut(block)
            .ok_or(MirFunctionBuildError::UnknownBlock(block))?;
        if block_data.terminator.is_some() {
            return Err(MirFunctionBuildError::AlreadyTerminated);
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
    ) -> Result<MirOperationId, MirFunctionBuildError> {
        if kind.produces_value() {
            return Err(MirFunctionBuildError::ValueKindUsedAsEffect);
        }
        let block_data = self
            .blocks
            .get_mut(block)
            .ok_or(MirFunctionBuildError::UnknownBlock(block))?;
        if block_data.terminator.is_some() {
            return Err(MirFunctionBuildError::AlreadyTerminated);
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
    ) -> Result<(), MirFunctionBuildError> {
        let block = self
            .blocks
            .get_mut(block)
            .ok_or(MirFunctionBuildError::UnknownBlock(block))?;
        if block.terminator.is_some() {
            return Err(MirFunctionBuildError::AlreadyTerminated);
        }
        block.terminator = Some(terminator);
        Ok(())
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
    ) -> Result<MirFunction, MirFunctionBuildError> {
        let blocks = self.blocks.try_finish_with(|block, draft| {
            let terminator = draft
                .terminator
                .ok_or(MirFunctionBuildError::UnterminatedBlock(block))?;
            Ok::<MirBlock, MirFunctionBuildError>(MirBlock::new(
                draft.parameters,
                draft.operations,
                terminator,
            ))
        })?;
        let function = MirFunction::new(
            self.item,
            self.parameters,
            self.result,
            MirFunctionDomains {
                locals: self.locals.finish(),
                drop_flags: self.drop_flags.finish(),
                places: self.places.finish(),
                values: self.values.finish(),
                operations: self.operations.finish(),
                blocks,
            },
            entry,
        );
        validate_function(&function, environment)?;
        Ok(function)
    }
}

#[derive(Debug)]
pub enum MirFunctionBuildError {
    UnknownBlock(MirBlockId),
    AlreadyTerminated,
    UnterminatedBlock(MirBlockId),
    EffectKindUsedAsValue,
    ValueKindUsedAsEffect,
    Validation(MirValidationError),
}

impl fmt::Display for MirFunctionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MIR function construction failed: {self:?}")
    }
}

impl std::error::Error for MirFunctionBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            Self::UnknownBlock(_)
            | Self::AlreadyTerminated
            | Self::UnterminatedBlock(_)
            | Self::EffectKindUsedAsValue
            | Self::ValueKindUsedAsEffect => None,
        }
    }
}

impl From<MirValidationError> for MirFunctionBuildError {
    fn from(error: MirValidationError) -> Self {
        Self::Validation(error)
    }
}
