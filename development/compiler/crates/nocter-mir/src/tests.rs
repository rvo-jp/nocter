use std::collections::BTreeMap;

use nocter_declarations::{
    FieldDeclaration, NominalTypeDeclaration, Parameter, StandardDeclarationRole,
    VariantDeclaration,
};
use nocter_model::{
    Arena, ArenaBuilder, BorrowCapability, BuiltinType, ExecutableItemId, FieldId, NominalTypeId,
    ParameterId, TypeId, TypeKind, TypeStore, VariantId,
};

use crate::{
    MirBinaryOperation, MirBranchTarget, MirConstant, MirFunctionBuildError, MirFunctionBuilder,
    MirLocalKind, MirOperationKind, MirPlaceRoot, MirProjection, MirProjectionKind, MirReadMode,
    MirTerminator, MirValidationEnvironment, MirValidationError,
};

struct TestEnvironment {
    types: TypeStore,
    items: Arena<ExecutableItemId, ()>,
    standard_nominals: BTreeMap<StandardDeclarationRole, NominalTypeId>,
}

impl TestEnvironment {
    fn new() -> (Self, ExecutableItemId) {
        Self::with_types(TypeStore::new())
    }

    fn with_types(types: TypeStore) -> (Self, ExecutableItemId) {
        let mut items = ArenaBuilder::new();
        let item = items.insert(());
        (
            Self {
                types,
                items: items.finish(),
                standard_nominals: BTreeMap::new(),
            },
            item,
        )
    }
}

#[test]
fn immutable_slots_preserve_the_write_authority_of_stored_borrows() {
    let mut types = TypeStore::new();
    let void = types.builtin(BuiltinType::Void);
    let i32_ = types.builtin(BuiltinType::I32);
    let borrow = types
        .intern(TypeKind::Borrow {
            capability: BorrowCapability::ReadWrite,
            referent: i32_,
        })
        .unwrap();
    let (environment, item) = TestEnvironment::with_types(types);
    let mut builder = MirFunctionBuilder::new(item, void);
    let parameter = builder.add_parameter(borrow, false);
    let place = builder.add_place(
        MirPlaceRoot::Local(parameter),
        [MirProjection::new(
            MirProjectionKind::BorrowDereference(BorrowCapability::ReadWrite),
            i32_,
        )],
        i32_,
    );
    let (entry, _) = builder.create_block([]);
    let value = builder
        .append_value(
            entry,
            i32_,
            MirOperationKind::Constant(MirConstant::Integer(1)),
        )
        .unwrap();
    builder
        .append_effect(
            entry,
            MirOperationKind::Store {
                destination: place,
                value,
            },
        )
        .unwrap();
    builder
        .terminate(entry, MirTerminator::Return(None))
        .unwrap();

    builder.finish(entry, &environment).unwrap();
}

#[test]
fn an_outer_readonly_borrow_remains_a_write_authority_ceiling() {
    let mut types = TypeStore::new();
    let void = types.builtin(BuiltinType::Void);
    let i32_ = types.builtin(BuiltinType::I32);
    let readwrite = types
        .intern(TypeKind::Borrow {
            capability: BorrowCapability::ReadWrite,
            referent: i32_,
        })
        .unwrap();
    let readonly = types
        .intern(TypeKind::Borrow {
            capability: BorrowCapability::Readonly,
            referent: readwrite,
        })
        .unwrap();
    let (environment, item) = TestEnvironment::with_types(types);
    let mut builder = MirFunctionBuilder::new(item, void);
    let parameter = builder.add_parameter(readonly, false);
    let place = builder.add_place(
        MirPlaceRoot::Local(parameter),
        [
            MirProjection::new(
                MirProjectionKind::BorrowDereference(BorrowCapability::Readonly),
                readwrite,
            ),
            MirProjection::new(
                MirProjectionKind::BorrowDereference(BorrowCapability::ReadWrite),
                i32_,
            ),
        ],
        i32_,
    );
    let (entry, _) = builder.create_block([]);
    let value = builder
        .append_value(
            entry,
            i32_,
            MirOperationKind::Constant(MirConstant::Integer(1)),
        )
        .unwrap();
    builder
        .append_effect(
            entry,
            MirOperationKind::Store {
                destination: place,
                value,
            },
        )
        .unwrap();
    builder
        .terminate(entry, MirTerminator::Return(None))
        .unwrap();

    assert!(matches!(
        builder.finish(entry, &environment),
        Err(MirFunctionBuildError::Validation(
            MirValidationError::OperationType(_)
        ))
    ));
}

impl MirValidationEnvironment for TestEnvironment {
    fn types(&self) -> &TypeStore {
        &self.types
    }

    fn contains_item(&self, item: ExecutableItemId) -> bool {
        self.items.get(item).is_some()
    }

    fn nominal_type(&self, _id: NominalTypeId) -> Option<&NominalTypeDeclaration> {
        None
    }

    fn field(&self, _id: FieldId) -> Option<&FieldDeclaration> {
        None
    }

    fn variant(&self, _id: VariantId) -> Option<&VariantDeclaration> {
        None
    }

    fn parameter(&self, _id: ParameterId) -> Option<&Parameter> {
        None
    }

    fn standard_nominal(&self, role: StandardDeclarationRole) -> Option<NominalTypeId> {
        self.standard_nominals.get(&role).copied()
    }

    fn closure_layout(
        &self,
        _item: ExecutableItemId,
    ) -> Option<&nocter_target_program::ExecutableClosureLayout> {
        None
    }

    fn closure_capture_type(
        &self,
        _closure_ty: TypeId,
        _capture: nocter_model::CaptureId,
    ) -> Option<TypeId> {
        None
    }
}

#[test]
fn region_operations_require_the_selected_standard_contract() {
    let mut nominal_ids = ArenaBuilder::<NominalTypeId, _>::new();
    let allocator = nominal_ids.insert(());
    let context = nominal_ids.insert(());
    let mut types = TypeStore::new();
    let void = types.builtin(BuiltinType::Void);
    let i32_ = types.builtin(BuiltinType::I32);
    let allocator_ty = types
        .intern(TypeKind::Nominal {
            definition: allocator,
            arguments: Box::new([]),
        })
        .unwrap();
    let context_ty = types
        .intern(TypeKind::Nominal {
            definition: context,
            arguments: Box::new([]),
        })
        .unwrap();
    let parent_ty = types
        .intern(TypeKind::Borrow {
            capability: BorrowCapability::Readonly,
            referent: allocator_ty,
        })
        .unwrap();
    let (mut environment, item) = TestEnvironment::with_types(types);
    environment.standard_nominals.extend([
        (StandardDeclarationRole::AbortingAllocator, allocator),
        (StandardDeclarationRole::AllocationContext, context),
    ]);

    let mut builder = MirFunctionBuilder::new(item, void);
    let parent = builder.add_parameter(parent_ty, false);
    let parent_place = builder.add_place(MirPlaceRoot::Local(parent), [], parent_ty);
    let region = builder.add_local(context_ty, MirLocalKind::Region, false);
    let region_place = builder.add_place(MirPlaceRoot::Local(region), [], context_ty);
    let (entry, _) = builder.create_block([]);
    let parent = builder
        .append_value(
            entry,
            parent_ty,
            MirOperationKind::Read {
                place: parent_place,
                mode: MirReadMode::Copy,
            },
        )
        .unwrap();
    let child = builder
        .append_value(entry, context_ty, MirOperationKind::CreateRegion { parent })
        .unwrap();
    builder
        .append_effect(
            entry,
            MirOperationKind::Initialize {
                destination: region_place,
                value: child,
            },
        )
        .unwrap();
    builder
        .append_effect(entry, MirOperationKind::ReleaseRegion { region })
        .unwrap();
    builder
        .terminate(entry, MirTerminator::Return(None))
        .unwrap();
    builder.finish(entry, &environment).unwrap();

    let mut invalid = MirFunctionBuilder::new(item, void);
    let (entry, _) = invalid.create_block([]);
    let parent = invalid
        .append_value(
            entry,
            i32_,
            MirOperationKind::Constant(MirConstant::Integer(0)),
        )
        .unwrap();
    invalid
        .append_value(entry, context_ty, MirOperationKind::CreateRegion { parent })
        .unwrap();
    invalid
        .terminate(entry, MirTerminator::Return(None))
        .unwrap();
    assert!(matches!(
        invalid.finish(entry, &environment),
        Err(MirFunctionBuildError::Validation(
            MirValidationError::OperationType(_)
        ))
    ));
}

#[test]
fn typed_block_parameters_close_a_diamond_cfg() {
    let (environment, item) = TestEnvironment::new();
    let bool_ = environment.types.builtin(BuiltinType::Bool);
    let i32_ = environment.types.builtin(BuiltinType::I32);
    let mut builder = MirFunctionBuilder::new(item, i32_);
    let (entry, _) = builder.create_block([]);
    let (left, _) = builder.create_block([]);
    let (right, _) = builder.create_block([]);
    let (join, join_parameters) = builder.create_block([i32_]);
    let condition = builder
        .append_value(
            entry,
            bool_,
            MirOperationKind::Constant(MirConstant::Bool(true)),
        )
        .unwrap();
    builder
        .terminate(
            entry,
            MirTerminator::Branch {
                condition,
                then_target: MirBranchTarget::new(left, []),
                else_target: MirBranchTarget::new(right, []),
            },
        )
        .unwrap();
    for (block, number) in [(left, 1), (right, 2)] {
        let value = builder
            .append_value(
                block,
                i32_,
                MirOperationKind::Constant(MirConstant::Integer(number)),
            )
            .unwrap();
        builder
            .terminate(
                block,
                MirTerminator::Goto(MirBranchTarget::new(join, [value])),
            )
            .unwrap();
    }
    builder
        .terminate(join, MirTerminator::Return(Some(join_parameters[0])))
        .unwrap();

    let function = builder.finish(entry, &environment).unwrap();

    assert_eq!(function.blocks().len(), 4);
    assert_eq!(function.values().len(), 4);
}

#[test]
fn edge_arguments_must_match_block_parameter_types() {
    let (environment, item) = TestEnvironment::new();
    let bool_ = environment.types.builtin(BuiltinType::Bool);
    let i32_ = environment.types.builtin(BuiltinType::I32);
    let mut builder = MirFunctionBuilder::new(item, i32_);
    let (entry, _) = builder.create_block([]);
    let (join, parameters) = builder.create_block([i32_]);
    let wrong = builder
        .append_value(
            entry,
            bool_,
            MirOperationKind::Constant(MirConstant::Bool(false)),
        )
        .unwrap();
    builder
        .terminate(
            entry,
            MirTerminator::Goto(MirBranchTarget::new(join, [wrong])),
        )
        .unwrap();
    builder
        .terminate(join, MirTerminator::Return(Some(parameters[0])))
        .unwrap();

    assert!(matches!(
        builder.finish(entry, &environment),
        Err(MirFunctionBuildError::Validation(
            MirValidationError::EdgeType { block, position: 0 }
        )) if block == join
    ));
}

#[test]
fn a_value_from_one_branch_does_not_dominate_its_sibling() {
    let (environment, item) = TestEnvironment::new();
    let bool_ = environment.types.builtin(BuiltinType::Bool);
    let i32_ = environment.types.builtin(BuiltinType::I32);
    let mut builder = MirFunctionBuilder::new(item, i32_);
    let (entry, _) = builder.create_block([]);
    let (left, _) = builder.create_block([]);
    let (right, _) = builder.create_block([]);
    let condition = builder
        .append_value(
            entry,
            bool_,
            MirOperationKind::Constant(MirConstant::Bool(true)),
        )
        .unwrap();
    builder
        .terminate(
            entry,
            MirTerminator::Branch {
                condition,
                then_target: MirBranchTarget::new(left, []),
                else_target: MirBranchTarget::new(right, []),
            },
        )
        .unwrap();
    let left_value = builder
        .append_value(
            left,
            i32_,
            MirOperationKind::Constant(MirConstant::Integer(1)),
        )
        .unwrap();
    builder
        .terminate(left, MirTerminator::Return(Some(left_value)))
        .unwrap();
    let one = builder
        .append_value(
            right,
            i32_,
            MirOperationKind::Constant(MirConstant::Integer(1)),
        )
        .unwrap();
    let invalid = builder
        .append_value(
            right,
            i32_,
            MirOperationKind::Binary {
                operation: MirBinaryOperation::Add,
                left: left_value,
                right: one,
            },
        )
        .unwrap();
    builder
        .terminate(right, MirTerminator::Return(Some(invalid)))
        .unwrap();

    assert!(matches!(
        builder.finish(entry, &environment),
        Err(MirFunctionBuildError::Validation(
            MirValidationError::ValueDoesNotDominate { value, block }
        )) if value == left_value && block == right
    ));
}

#[test]
fn builder_rejects_unterminated_blocks_before_validation() {
    let (environment, item) = TestEnvironment::new();
    let void = environment.types.builtin(BuiltinType::Void);
    let mut builder = MirFunctionBuilder::new(item, void);
    let (entry, _) = builder.create_block([]);

    assert!(matches!(
        builder.finish(entry, &environment),
        Err(MirFunctionBuildError::UnterminatedBlock(block)) if block == entry
    ));
}

#[test]
fn drop_flags_are_explicit_cfg_conditions() {
    let (environment, item) = TestEnvironment::new();
    let void = environment.types.builtin(BuiltinType::Void);
    let i32_ = environment.types.builtin(BuiltinType::I32);
    let mut builder = MirFunctionBuilder::new(item, void);
    let local = builder.add_local(i32_, MirLocalKind::User, true);
    let place = builder.add_place(MirPlaceRoot::Local(local), [], i32_);
    let flag = builder.add_drop_flag(place, false);
    let (entry, _) = builder.create_block([]);
    let (initialized, _) = builder.create_block([]);
    let (uninitialized, _) = builder.create_block([]);
    builder
        .terminate(
            entry,
            MirTerminator::BranchDropFlag {
                flag,
                initialized: MirBranchTarget::new(initialized, []),
                uninitialized: MirBranchTarget::new(uninitialized, []),
            },
        )
        .unwrap();
    for block in [initialized, uninitialized] {
        builder
            .terminate(block, MirTerminator::Return(None))
            .unwrap();
    }

    let function = builder.finish(entry, &environment).unwrap();

    assert_eq!(function.drop_flags().len(), 1);
}

#[test]
fn identical_typed_places_share_one_identity() {
    let (environment, item) = TestEnvironment::new();
    let void = environment.types.builtin(BuiltinType::Void);
    let i32_ = environment.types.builtin(BuiltinType::I32);
    let mut builder = MirFunctionBuilder::new(item, void);
    let local = builder.add_local(i32_, MirLocalKind::User, true);

    let first = builder.add_place(MirPlaceRoot::Local(local), [], i32_);
    let second = builder.add_place(MirPlaceRoot::Local(local), [], i32_);

    assert_eq!(first, second);
}

#[test]
fn rejecting_a_second_terminator_preserves_the_first() {
    let (environment, item) = TestEnvironment::new();
    let void = environment.types.builtin(BuiltinType::Void);
    let mut builder = MirFunctionBuilder::new(item, void);
    let (entry, _) = builder.create_block([]);
    builder
        .terminate(entry, MirTerminator::Return(None))
        .unwrap();

    assert!(matches!(
        builder.terminate(entry, MirTerminator::Trap),
        Err(MirFunctionBuildError::AlreadyTerminated)
    ));
    let function = builder.finish(entry, &environment).unwrap();
    assert!(matches!(
        function.blocks().get(entry).unwrap().terminator(),
        MirTerminator::Return(None)
    ));
}
