use std::collections::{BTreeMap, BTreeSet};

use nocter_declarations::StandardDeclarationRole;
use nocter_model::{
    Arena, ArenaBuilder, BorrowCapability, BuiltinType, ExecutableItemId, MirBlockId,
    NominalTypeId, TypeId, TypeKind, TypeStore, TypeTransaction,
};
use nocter_runtime_contract::{RuntimeTypeRepresentation, RuntimeTypeRepresentationTable};

use crate::{
    MirBinaryOperation, MirBodyBuildError, MirBodyBuilder, MirBranchTarget, MirCall,
    MirCallAllocation, MirCallTarget, MirConstant, MirDestructionKind, MirDestructionPlan,
    MirFunction, MirFunctionBuilder, MirLocalKind, MirOperationKind, MirPackArgument,
    MirPackSegment, MirPlaceRoot, MirProjection, MirProjectionKind, MirReadMode, MirTerminator,
    MirValidationEnvironment, MirValidationError,
};

struct TestEnvironment {
    types: TypeStore,
    items: Arena<ExecutableItemId, ()>,
    allocation_items: BTreeSet<ExecutableItemId>,
    standard_nominals: BTreeMap<StandardDeclarationRole, NominalTypeId>,
    pack_inputs: BTreeMap<ExecutableItemId, (TypeId, TypeId)>,
    representations: RuntimeTypeRepresentationTable,
}

impl TestEnvironment {
    fn new() -> (Self, ExecutableItemId) {
        Self::with_types(TypeStore::new().transaction())
    }

    fn with_types(types: TypeTransaction) -> (Self, ExecutableItemId) {
        let mut items = ArenaBuilder::new();
        let item = items.insert(());
        (
            Self {
                types: types.freeze(),
                items: items.finish(),
                allocation_items: BTreeSet::new(),
                standard_nominals: BTreeMap::new(),
                pack_inputs: BTreeMap::new(),
                representations: RuntimeTypeRepresentationTable::default(),
            },
            item,
        )
    }
}

#[test]
fn immutable_slots_preserve_the_write_authority_of_stored_borrows() {
    let mut types = TypeStore::new().transaction();
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

    finish_validated(builder, entry, &environment).unwrap();
}

#[test]
fn an_outer_readonly_borrow_remains_a_write_authority_ceiling() {
    let mut types = TypeStore::new().transaction();
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
        finish_validated(builder, entry, &environment),
        Err(MirValidationError::OperationType(_))
    ));
}

impl MirValidationEnvironment for TestEnvironment {
    fn types(&self) -> &TypeStore {
        &self.types
    }

    fn contains_item(&self, item: ExecutableItemId) -> bool {
        self.items.get(item).is_some()
    }

    fn item_accepts_allocation_override(&self, item: ExecutableItemId) -> bool {
        self.allocation_items.contains(&item)
    }

    fn item_pack_input(&self, item: ExecutableItemId) -> Option<(TypeId, TypeId)> {
        self.pack_inputs.get(&item).copied()
    }

    fn type_representation(&self, ty: TypeId) -> Option<&RuntimeTypeRepresentation> {
        self.representations.get(ty)
    }

    fn allocation_context_nominal(&self) -> Option<NominalTypeId> {
        self.standard_nominals
            .get(&StandardDeclarationRole::AllocationContext)
            .copied()
    }

    fn aborting_allocator_nominal(&self) -> Option<NominalTypeId> {
        self.standard_nominals
            .get(&StandardDeclarationRole::AbortingAllocator)
            .copied()
    }

    fn closure_layout(
        &self,
        _item: ExecutableItemId,
    ) -> Option<&nocter_target_program::ExecutableClosureLayout> {
        None
    }
}

fn finish_validated(
    builder: MirFunctionBuilder,
    entry: MirBlockId,
    environment: &impl MirValidationEnvironment,
) -> Result<MirFunction, MirValidationError> {
    let function = builder
        .finish(entry)
        .expect("validation fixture must be structurally complete");
    crate::validate_function(&function, environment)?;
    Ok(function)
}

#[test]
fn call_allocation_overrides_require_a_literal_item_and_selected_context_role() {
    let mut nominal_ids = ArenaBuilder::<NominalTypeId, _>::new();
    let allocator = nominal_ids.insert(());
    let mut types = TypeStore::new().transaction();
    let void = types.builtin(BuiltinType::Void);
    let i32_ = types.builtin(BuiltinType::I32);
    let allocator_ty = types
        .intern(TypeKind::Nominal {
            definition: allocator,
            arguments: Box::new([]),
        })
        .unwrap();
    let (mut environment, item) = TestEnvironment::with_types(types);
    environment.allocation_items.insert(item);
    environment
        .standard_nominals
        .insert(StandardDeclarationRole::AbortingAllocator, allocator);

    let build = |place_ty, accepted: bool| {
        let environment = TestEnvironment {
            types: environment.types.clone(),
            items: environment.items.clone(),
            allocation_items: accepted.then_some(item).into_iter().collect(),
            standard_nominals: environment.standard_nominals.clone(),
            pack_inputs: environment.pack_inputs.clone(),
            representations: environment.representations.clone(),
        };
        let mut builder = MirFunctionBuilder::new(item, void);
        let parameter = builder.add_parameter(place_ty, false);
        let place = builder.add_place(MirPlaceRoot::Local(parameter), [], place_ty);
        let (entry, _) = builder.create_block([]);
        builder
            .append_value(
                entry,
                void,
                MirOperationKind::Call(MirCall::with_allocation(
                    MirCallTarget::Direct(item),
                    [],
                    MirCallAllocation::Explicit(place),
                )),
            )
            .unwrap();
        builder
            .terminate(entry, MirTerminator::Return(None))
            .unwrap();
        finish_validated(builder, entry, &environment)
    };

    assert!(build(allocator_ty, true).is_ok());
    for result in [build(allocator_ty, false), build(i32_, true)] {
        assert!(matches!(result, Err(MirValidationError::OperationType(_))));
    }
}

#[test]
fn pack_inputs_require_exact_types_and_destruction_on_every_return() {
    let mut types = TypeStore::new().transaction();
    let void = types.builtin(BuiltinType::Void);
    let i32_ = types.builtin(BuiltinType::I32);
    let next = types.intern(TypeKind::Optional(i32_)).unwrap();
    let (mut environment, item) = TestEnvironment::with_types(types);
    environment.pack_inputs.insert(item, (i32_, next));

    let build = |input: crate::MirPackInput, destroys: bool| {
        let mut builder = MirFunctionBuilder::new(item, void);
        builder.set_pack_input(input).unwrap();
        let (entry, _) = builder.create_block([]);
        builder
            .append_value(entry, next, MirOperationKind::PackNext)
            .unwrap();
        if destroys {
            builder
                .append_effect(entry, MirOperationKind::DestroyPack)
                .unwrap();
        }
        builder
            .terminate(entry, MirTerminator::Return(None))
            .unwrap();
        finish_validated(builder, entry, &environment)
    };

    assert!(build(crate::MirPackInput::new(i32_, next), true).is_ok());
    assert!(matches!(
        build(crate::MirPackInput::new(i32_, next), false),
        Err(MirValidationError::InvalidPackExit(_))
    ));
    assert!(matches!(
        build(crate::MirPackInput::new(void, next), true),
        Err(MirValidationError::InvalidPackInput(_))
    ));
}

#[test]
fn pack_calls_require_the_exact_hidden_lane_and_validate_deferred_cleanup() {
    let mut types = TypeStore::new().transaction();
    let i32_ = types.builtin(BuiltinType::I32);
    let next = types.intern(TypeKind::Optional(i32_)).unwrap();
    let mut items = ArenaBuilder::new();
    let caller = items.insert(());
    let literal = items.insert(());
    let environment = TestEnvironment {
        types: types.freeze(),
        items: items.finish(),
        allocation_items: BTreeSet::new(),
        standard_nominals: BTreeMap::new(),
        pack_inputs: BTreeMap::from([(literal, (i32_, next))]),
        representations: RuntimeTypeRepresentationTable::default(),
    };

    assert!(build_pack_call(&environment, caller, literal, i32_, next, true, None).is_ok());

    assert!(matches!(
        build_pack_call(&environment, caller, literal, i32_, next, false, None),
        Err(MirValidationError::OperationType(_))
    ));

    let invalid_plan = MirDestructionPlan::new(
        i32_,
        MirDestructionKind::Optional(Box::new(MirDestructionPlan::new(
            i32_,
            MirDestructionKind::Closure(Box::new([])),
        ))),
    );
    assert!(matches!(
        build_pack_call(
            &environment,
            caller,
            literal,
            i32_,
            next,
            true,
            Some(invalid_plan),
        ),
        Err(MirValidationError::InvalidDestruction(ty)) if ty == i32_
    ));
}

fn build_pack_call(
    environment: &TestEnvironment,
    caller: ExecutableItemId,
    literal: ExecutableItemId,
    element: TypeId,
    next: TypeId,
    include_pack: bool,
    destruction: Option<MirDestructionPlan>,
) -> Result<crate::MirFunction, MirValidationError> {
    let mut builder = MirFunctionBuilder::new(caller, element);
    let (entry, _) = builder.create_block([]);
    let call = if include_pack {
        let length = builder
            .append_value(
                entry,
                environment.types.builtin(BuiltinType::Usize),
                MirOperationKind::Constant(MirConstant::Integer(1)),
            )
            .unwrap();
        let value = builder
            .append_value(
                entry,
                element,
                MirOperationKind::Constant(MirConstant::Integer(7)),
            )
            .unwrap();
        let pack = MirPackArgument::new(
            element,
            next,
            length,
            [MirPackSegment::Value { value, destruction }],
        );
        MirCall::with_pack(
            MirCallTarget::Direct(literal),
            [],
            crate::MirCallPack::Prepared(pack),
            MirCallAllocation::Inherit,
        )
    } else {
        MirCall::new(MirCallTarget::Direct(literal), [])
    };
    let result = builder
        .append_value(entry, element, MirOperationKind::Call(call))
        .unwrap();
    builder
        .terminate(entry, MirTerminator::Return(Some(result)))
        .unwrap();
    finish_validated(builder, entry, environment)
}

struct RegionTestContext {
    environment: TestEnvironment,
    item: ExecutableItemId,
    void: TypeId,
    i32_: TypeId,
    context: TypeId,
    parent: TypeId,
}

fn region_test_context() -> RegionTestContext {
    let mut nominal_ids = ArenaBuilder::<NominalTypeId, _>::new();
    let allocator = nominal_ids.insert(());
    let context = nominal_ids.insert(());
    let mut types = TypeStore::new().transaction();
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
    RegionTestContext {
        environment,
        item,
        void,
        i32_,
        context: context_ty,
        parent: parent_ty,
    }
}

#[test]
fn region_operations_require_the_selected_standard_contract() {
    let RegionTestContext {
        environment,
        item,
        void,
        i32_,
        context: context_ty,
        parent: parent_ty,
    } = region_test_context();

    let mut builder = MirFunctionBuilder::new(item, void);
    let parent = builder.add_parameter(parent_ty, false);
    let parent_place = builder.add_place(MirPlaceRoot::Local(parent), [], parent_ty);
    let region = builder.add_local(context_ty, MirLocalKind::Region, false);
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
    builder
        .append_effect(entry, MirOperationKind::CreateRegion { parent, region })
        .unwrap();
    builder
        .append_effect(entry, MirOperationKind::ReleaseRegion { region })
        .unwrap();
    builder
        .terminate(entry, MirTerminator::Return(None))
        .unwrap();
    finish_validated(builder, entry, &environment).unwrap();

    let mut invalid = MirFunctionBuilder::new(item, void);
    let region = invalid.add_local(context_ty, MirLocalKind::Region, false);
    let (entry, _) = invalid.create_block([]);
    let parent = invalid
        .append_value(
            entry,
            i32_,
            MirOperationKind::Constant(MirConstant::Integer(0)),
        )
        .unwrap();
    invalid
        .append_effect(entry, MirOperationKind::CreateRegion { parent, region })
        .unwrap();
    invalid
        .terminate(entry, MirTerminator::Return(None))
        .unwrap();
    assert!(matches!(
        finish_validated(invalid, entry, &environment),
        Err(MirValidationError::OperationType(_))
    ));
}

#[test]
fn region_flow_rejects_a_live_context_at_return() {
    let RegionTestContext {
        environment,
        item,
        void,
        context: context_ty,
        parent: parent_ty,
        ..
    } = region_test_context();
    let mut unbalanced = MirFunctionBuilder::new(item, void);
    let parent = unbalanced.add_parameter(parent_ty, false);
    let parent_place = unbalanced.add_place(MirPlaceRoot::Local(parent), [], parent_ty);
    let region = unbalanced.add_local(context_ty, MirLocalKind::Region, false);
    let (entry, _) = unbalanced.create_block([]);
    let parent = unbalanced
        .append_value(
            entry,
            parent_ty,
            MirOperationKind::Read {
                place: parent_place,
                mode: MirReadMode::Copy,
            },
        )
        .unwrap();
    unbalanced
        .append_effect(entry, MirOperationKind::CreateRegion { parent, region })
        .unwrap();
    unbalanced
        .terminate(entry, MirTerminator::Return(None))
        .unwrap();
    assert!(matches!(
        finish_validated(unbalanced, entry, &environment),
        Err(MirValidationError::InvalidRegionFlow {
            region: Some(actual),
            ..
        }) if actual == region
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

    let function = finish_validated(builder, entry, &environment).unwrap();

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
        finish_validated(builder, entry, &environment),
        Err(MirValidationError::EdgeType { block, position: 0 }) if block == join
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
        finish_validated(builder, entry, &environment),
        Err(MirValidationError::ValueDoesNotDominate { value, block })
            if value == left_value && block == right
    ));
}

#[test]
fn builder_rejects_unterminated_blocks_before_validation() {
    let (environment, item) = TestEnvironment::new();
    let void = environment.types.builtin(BuiltinType::Void);
    let mut builder = MirFunctionBuilder::new(item, void);
    let (entry, _) = builder.create_block([]);

    assert!(matches!(
        builder.finish(entry),
        Err(MirBodyBuildError::UnterminatedBlock(block)) if block == entry
    ));
}

#[test]
fn functions_and_process_roots_have_disjoint_terminal_contracts() {
    let (environment, item) = TestEnvironment::new();
    let void = environment.types.builtin(BuiltinType::Void);
    let mut function = MirFunctionBuilder::new(item, void);
    let (entry, _) = function.create_block([]);
    function
        .terminate(entry, MirTerminator::Exit(None))
        .unwrap();
    assert!(matches!(
        finish_validated(function, entry, &environment),
        Err(MirValidationError::InvalidReturn(block)) if block == entry
    ));

    let mut root = MirBodyBuilder::new();
    let (entry, _) = root.create_block([]);
    root.terminate(entry, MirTerminator::Return(None)).unwrap();
    let body = root.finish(entry).unwrap();
    assert_eq!(
        crate::validate::validate_root(&body, &environment),
        Err(MirValidationError::InvalidRootTerminator(entry))
    );
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

    let function = finish_validated(builder, entry, &environment).unwrap();

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
        Err(MirBodyBuildError::AlreadyTerminated)
    ));
    let function = finish_validated(builder, entry, &environment).unwrap();
    assert!(matches!(
        function.blocks().get(entry).unwrap().terminator(),
        MirTerminator::Return(None)
    ));
}
