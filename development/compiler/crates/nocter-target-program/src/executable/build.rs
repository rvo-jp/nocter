use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use nocter_checking::{
    ConcreteDestructionKind, ConcreteDestructionPlan, ConcreteDispatchResolver, DropSelection,
    GenericArgument, GenericArguments, ResolvedDispatchPlan, ResolvedDispatchStep, StaticSelection,
    TypeSubstitution,
};
use nocter_model::{
    ArenaBuilder, BodyId, BodyNodeId, CallableCapability, ExecutableItemId, TypeId, TypeKind,
};

use super::signature::{build_signature, callable_signature};
use super::{
    ExecutableBody, ExecutableBorrowEdge, ExecutableClosureEdge, ExecutableDestructionEdge,
    ExecutableDispatchEdge, ExecutableDispatchPlan, ExecutableDispatchStep, ExecutableDropEdge,
    ExecutableItem, ExecutableItemKey, ExecutablePrimitiveCall, ExecutableProgram,
    ExecutableProgramError, ExecutableRoot, ExecutableTestCase, ExecutableTypeEdge,
};

mod primitive;
mod sequence;
use crate::{
    CallableInstanceKey, CheckedDestruction, ClosureInstanceKey, DropInstanceKey, TargetProgram,
    collect_body_dependencies, select_executable_entry, select_test_target,
};

pub(super) fn build_executable(
    target: Arc<TargetProgram>,
    selected: nocter_model::PackageTargetId,
) -> Result<ExecutableProgram, ExecutableProgramError> {
    let entry = select_executable_entry(&target, selected)?;
    let entry_key = ExecutableItemKey::Callable(CallableInstanceKey::for_entry(&target, entry)?);
    let frozen = ExecutableClosureBuilder::new(&target).close([entry_key.clone()])?;
    let entry_item = frozen.item_id(&entry_key)?;
    let runtime = super::build_runtime_environment(
        &frozen.types,
        frozen.type_representations,
        target.toolchain().abi(),
    )?;
    let semantic_environment =
        super::semantic_environment::ExecutableSemanticEnvironment::freeze(target.as_ref());
    Ok(ExecutableProgram {
        target,
        semantic_environment,
        types: frozen.types,
        items: frozen.items,
        item_ids: frozen.item_ids,
        runtime,
        root: ExecutableRoot::Process {
            target: selected,
            entry: entry_item,
            result: entry.process_result(),
        },
    })
}

pub(super) fn build_tests(
    target: Arc<TargetProgram>,
    selected: nocter_model::PackageTargetId,
) -> Result<ExecutableProgram, ExecutableProgramError> {
    let selection = select_test_target(&target, selected)?;
    build_selected_tests(target, &selection)
}

pub(super) fn build_selected_tests(
    target: Arc<TargetProgram>,
    selection: &crate::SelectedTestTarget,
) -> Result<ExecutableProgram, ExecutableProgramError> {
    let roots = selection
        .tests()
        .iter()
        .map(|test| ExecutableItemKey::Test(test.declaration()))
        .collect::<Vec<_>>();
    let frozen = ExecutableClosureBuilder::new(&target).close(roots.iter().cloned())?;
    let cases = selection
        .tests()
        .iter()
        .zip(roots)
        .map(|(test, key)| {
            let name = target
                .checked()
                .graph()
                .symbols()
                .spelling(test.name())
                .ok_or(ExecutableProgramError::MissingTestName(test.declaration()))?;
            Ok(ExecutableTestCase {
                declaration: test.declaration(),
                name: name.into(),
                item: frozen.item_id(&key)?,
            })
        })
        .collect::<Result<Vec<_>, ExecutableProgramError>>()?;
    let runtime = super::build_runtime_environment(
        &frozen.types,
        frozen.type_representations,
        target.toolchain().abi(),
    )?;
    let semantic_environment =
        super::semantic_environment::ExecutableSemanticEnvironment::freeze(target.as_ref());
    Ok(ExecutableProgram {
        target,
        semantic_environment,
        types: frozen.types,
        items: frozen.items,
        item_ids: frozen.item_ids,
        runtime,
        root: ExecutableRoot::Tests {
            target: selection.target(),
            cases: cases.into_boxed_slice(),
        },
    })
}

struct FrozenClosure {
    types: nocter_model::TypeStore,
    items: nocter_model::Arena<ExecutableItemId, ExecutableItem>,
    item_ids: BTreeMap<ExecutableItemKey, ExecutableItemId>,
    type_representations: super::RuntimeTypeRepresentationTable,
}

impl FrozenClosure {
    fn item_id(&self, key: &ExecutableItemKey) -> Result<ExecutableItemId, ExecutableProgramError> {
        self.item_ids
            .get(key)
            .copied()
            .ok_or_else(|| ExecutableProgramError::UnknownItem(key.clone()))
    }
}

struct ExecutableClosureBuilder<'program> {
    target: &'program TargetProgram,
    resolver: ConcreteDispatchResolver<'program>,
    known: BTreeSet<ExecutableItemKey>,
    pending: BTreeSet<ExecutableItemKey>,
    items: BTreeMap<ExecutableItemKey, DraftItem>,
}

impl<'program> ExecutableClosureBuilder<'program> {
    fn new(target: &'program TargetProgram) -> Self {
        Self {
            target,
            resolver: ConcreteDispatchResolver::new(target.checked()),
            known: BTreeSet::new(),
            pending: BTreeSet::new(),
            items: BTreeMap::new(),
        }
    }

    fn close(
        mut self,
        roots: impl IntoIterator<Item = ExecutableItemKey>,
    ) -> Result<FrozenClosure, ExecutableProgramError> {
        for root in roots {
            self.enqueue(root);
        }
        while let Some(key) = self.pending.pop_first() {
            let item = self.build_item(&key)?;
            if self.items.insert(key.clone(), item).is_some() {
                return Err(ExecutableProgramError::DuplicateItem(key));
            }
        }
        let closure_layouts = self
            .items
            .values()
            .filter_map(|item| item.closure.clone())
            .collect::<Vec<_>>();
        let type_representations = super::type_representation::close_type_representations(
            self.target,
            &mut self.resolver,
            &closure_layouts,
        )?;
        let types = self.resolver.into_types();
        freeze_items(self.items, types, type_representations)
    }

    fn enqueue(&mut self, key: ExecutableItemKey) {
        if self.known.insert(key.clone()) {
            self.pending.insert(key);
        }
    }

    fn build_item(&mut self, key: &ExecutableItemKey) -> Result<DraftItem, ExecutableProgramError> {
        let context = item_context(self.target, key)?;
        let signature = build_signature(self.target, &mut self.resolver, key)?;
        let accepts_allocation_override = accepts_allocation_override(self.target, key);
        let dependencies = collect_body_dependencies(self.target, context.body, context.root)?;
        let substitution = item_substitution(key);
        let mut drops = BTreeMap::new();
        let mut dispatches = Vec::new();
        for selection in dependencies.selections() {
            let plan = self.resolver.resolve(selection, &substitution)?;
            dispatches.push(DraftDispatchEdge {
                source: selection.clone(),
                plan: self.convert_dispatch(&plan, &mut drops)?,
            });
        }

        let mut closures = Vec::new();
        for closure in dependencies.closures().iter().copied() {
            let key = ExecutableItemKey::Closure(ClosureInstanceKey::new_in(
                self.target,
                self.resolver.types(),
                closure,
                item_generic_arguments(key),
            )?);
            self.enqueue(key.clone());
            closures.push((closure, key));
        }

        for selection in dependencies.drop_selections() {
            let selection = self.specialize_drop(selection, &substitution)?;
            self.record_drop(selection, &mut drops)?;
        }

        let types = dependencies
            .types()
            .iter()
            .copied()
            .map(|source| {
                self.resolver
                    .specialize_type(source, &substitution)
                    .map(|concrete| ExecutableTypeEdge { source, concrete })
                    .map_err(Into::into)
            })
            .collect::<Result<Vec<_>, ExecutableProgramError>>()?;

        let prepared_borrows = self.specialize_prepared_borrows(&dependencies, &substitution)?;
        let sequences = self.specialize_sequence_plans(
            context.body,
            &dependencies,
            &substitution,
            &dispatches,
            &mut drops,
        )?;
        let argument_packs = self.specialize_call_argument_packs(
            context.body,
            &dependencies,
            &substitution,
            &dispatches,
            signature.pack(),
            &mut drops,
        )?;
        let closure = self.specialize_closure_layout(key, &substitution)?;

        let mut destructions = Vec::new();
        for source in dependencies.destructions() {
            let plan = match source {
                CheckedDestruction::Complete(ty) => {
                    self.resolver.resolve_destruction(*ty, &substitution)?
                }
                CheckedDestruction::EnumResidual {
                    ty,
                    variant,
                    payload,
                } => self
                    .resolver
                    .resolve_enum_residual(*ty, *variant, payload, &substitution)?,
            };
            if let Some(plan) = plan {
                let mut selections = BTreeSet::new();
                collect_drops(&plan, &mut selections);
                for selection in selections {
                    self.record_drop(selection, &mut drops)?;
                }
                destructions.push((source.clone(), plan));
            }
        }

        Ok(DraftItem {
            signature,
            accepts_allocation_override,
            closure,
            body: context.body,
            root: context.root,
            nodes: dependencies.nodes().to_vec(),
            dispatches,
            closures,
            drops: drops.into_iter().collect(),
            types,
            prepared_borrows,
            destructions,
            sequences,
            argument_packs,
        })
    }

    fn specialize_closure_layout(
        &mut self,
        key: &ExecutableItemKey,
        substitution: &TypeSubstitution,
    ) -> Result<Option<super::ExecutableClosureLayout>, ExecutableProgramError> {
        let ExecutableItemKey::Closure(key) = key else {
            return Ok(None);
        };
        let definition = self
            .target
            .checked()
            .closures()
            .get(key.closure())
            .ok_or_else(|| {
                ExecutableProgramError::UnknownItem(ExecutableItemKey::Closure(key.clone()))
            })?;
        let ty = self
            .resolver
            .specialize_type(definition.ty(), substitution)?;
        let captures = definition
            .environment()
            .iter()
            .copied()
            .map(|capture| {
                self.resolver
                    .specialize_type(capture.ty(), substitution)
                    .map(|ty| super::ExecutableClosureCapture::new(capture.binding(), ty))
                    .map_err(Into::into)
            })
            .collect::<Result<Vec<_>, ExecutableProgramError>>()?;
        Ok(Some(super::ExecutableClosureLayout::new(
            key.closure(),
            ty,
            definition.signature().capability(),
            captures,
        )))
    }

    fn specialize_prepared_borrows(
        &mut self,
        dependencies: &crate::CheckedBodyDependencies,
        substitution: &TypeSubstitution,
    ) -> Result<Vec<ExecutableBorrowEdge>, ExecutableProgramError> {
        dependencies
            .prepared_borrows()
            .iter()
            .copied()
            .map(|borrow| {
                let source = borrow.source();
                let referent = self.resolver.specialize_type(source, substitution)?;
                let concrete = self
                    .resolver
                    .intern_concrete(nocter_model::TypeKind::Borrow {
                        capability: borrow.capability(),
                        referent,
                    })?;
                Ok(ExecutableBorrowEdge {
                    source,
                    capability: borrow.capability(),
                    concrete,
                })
            })
            .collect()
    }

    fn convert_dispatch(
        &mut self,
        plan: &ResolvedDispatchPlan,
        drops: &mut BTreeMap<DropSelection, ExecutableItemKey>,
    ) -> Result<DraftDispatchPlan, ExecutableProgramError> {
        Ok(match plan {
            ResolvedDispatchPlan::Invocation(step) => {
                DraftDispatchPlan::Invocation(self.convert_dispatch_step(step, drops)?)
            }
            ResolvedDispatchPlan::OpaqueInvocation {
                receiver,
                operation,
            } => DraftDispatchPlan::OpaqueInvocation {
                receiver: super::ExecutableOpaqueReceiver {
                    definition: receiver.definition(),
                    opaque: receiver.opaque(),
                    witness: receiver.witness(),
                    source: receiver.source(),
                    target: receiver.target(),
                },
                operation: self.convert_dispatch_step(operation, drops)?,
            },
            ResolvedDispatchPlan::Comparison {
                left_coercion,
                right_coercion,
                operation,
            } => DraftDispatchPlan::Comparison {
                left_coercion: left_coercion
                    .as_ref()
                    .map(|step| self.convert_dispatch_step(step, drops))
                    .transpose()?,
                right_coercion: right_coercion
                    .as_ref()
                    .map(|step| self.convert_dispatch_step(step, drops))
                    .transpose()?,
                operation: self.convert_dispatch_step(operation, drops)?,
            },
            ResolvedDispatchPlan::Index {
                receiver_coercion,
                operation,
            } => DraftDispatchPlan::Index {
                receiver_coercion: receiver_coercion
                    .as_ref()
                    .map(|step| self.convert_dispatch_step(step, drops))
                    .transpose()?,
                operation: self.convert_dispatch_step(operation, drops)?,
            },
        })
    }

    fn convert_dispatch_step(
        &mut self,
        step: &ResolvedDispatchStep,
        drops: &mut BTreeMap<DropSelection, ExecutableItemKey>,
    ) -> Result<DraftDispatchStep, ExecutableProgramError> {
        match step {
            ResolvedDispatchStep::Direct(dispatch) => {
                let callable_key = CallableInstanceKey::new_with_interface_self(
                    self.target,
                    self.resolver.types(),
                    dispatch.callable(),
                    dispatch.generic_arguments().clone(),
                    dispatch.interface_self(),
                )?;
                let declaration = self
                    .target
                    .checked()
                    .graph()
                    .declarations()
                    .callables()
                    .get(dispatch.callable())
                    .ok_or(ExecutableProgramError::BodylessCallable(
                        dispatch.callable(),
                    ))?;
                if declaration.body().is_some() {
                    let key = ExecutableItemKey::Callable(callable_key);
                    self.enqueue(key.clone());
                    Ok(DraftDispatchStep::Direct(key))
                } else if let Some(role) = self
                    .target
                    .toolchain()
                    .primitives()
                    .role(dispatch.callable())
                {
                    let signature = callable_signature(
                        self.target,
                        &mut self.resolver,
                        &callable_key,
                        &callable_key.substitution(),
                    )?;
                    Ok(DraftDispatchStep::StandardPrimitive(
                        ExecutablePrimitiveCall {
                            role,
                            generic_arguments: callable_key.generic_arguments().clone(),
                            signature,
                            dependency: self.specialize_primitive_dependency(
                                role,
                                callable_key.generic_arguments(),
                                drops,
                            )?,
                        },
                    ))
                } else {
                    Err(ExecutableProgramError::BodylessCallable(
                        dispatch.callable(),
                    ))
                }
            }
            ResolvedDispatchStep::Primitive(primitive) => {
                Ok(DraftDispatchStep::StructuralPrimitive(primitive.clone()))
            }
            ResolvedDispatchStep::CallableValue { subject, contract } => {
                self.convert_callable_value(*subject, contract, drops)
            }
        }
    }

    fn convert_callable_value(
        &mut self,
        subject: TypeId,
        contract: &nocter_model::CallableContract,
        drops: &mut BTreeMap<DropSelection, ExecutableItemKey>,
    ) -> Result<DraftDispatchStep, ExecutableProgramError> {
        let (closure, arguments) = match self.resolver.types().get(subject) {
            Some(TypeKind::Closure {
                definition,
                arguments,
            }) => (*definition, arguments.clone()),
            Some(_) | None => {
                return Err(ExecutableProgramError::InvalidCallableInvocation(subject));
            }
        };
        let definition = self
            .target
            .checked()
            .closures()
            .get(closure)
            .cloned()
            .ok_or(ExecutableProgramError::InvalidCallableInvocation(subject))?;
        let domain = self
            .target
            .checked()
            .graph()
            .declarations()
            .body_generic_domain(definition.owner())
            .ok_or(ExecutableProgramError::InvalidCallableInvocation(subject))?;
        if domain.len() != arguments.len() {
            return Err(ExecutableProgramError::InvalidCallableInvocation(subject));
        }
        let generic_arguments = GenericArguments::new(
            domain
                .iter()
                .copied()
                .zip(arguments.iter().copied())
                .map(|(parameter, ty)| GenericArgument::new(parameter, ty)),
        )
        .map_err(|duplicate| ExecutableProgramError::DuplicateGeneric(duplicate.parameter()))?;
        let key = ClosureInstanceKey::new_in(
            self.target,
            self.resolver.types(),
            closure,
            generic_arguments,
        )?;
        let substitution = key.substitution();
        let parameters = definition
            .signature()
            .parameters()
            .iter()
            .copied()
            .map(|parameter| self.resolver.specialize_type(parameter.ty(), &substitution))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self
            .resolver
            .specialize_type(definition.signature().result(), &substitution)?;
        if !contract
            .capability()
            .permits(definition.signature().capability())
            || parameters != contract.parameters()
            || contract.pack().is_some()
            || result != contract.result()
        {
            return Err(ExecutableProgramError::InvalidCallableInvocation(subject));
        }
        let post_call_destruction = if contract.capability() == CallableCapability::Owned
            && definition.signature().capability() != CallableCapability::Owned
        {
            self.resolver
                .resolve_destruction(subject, &TypeSubstitution::default())?
        } else {
            None
        };
        if let Some(plan) = &post_call_destruction {
            let mut selections = BTreeSet::new();
            collect_drops(plan, &mut selections);
            for selection in selections {
                self.record_drop(selection, drops)?;
            }
        }
        let body = ExecutableItemKey::Closure(key);
        self.enqueue(body.clone());
        Ok(DraftDispatchStep::CallableValue(DraftCallableInvocation {
            subject,
            contract: contract.clone(),
            body,
            post_call_destruction,
        }))
    }

    fn specialize_drop(
        &mut self,
        selection: &DropSelection,
        substitution: &TypeSubstitution,
    ) -> Result<DropSelection, ExecutableProgramError> {
        let arguments = selection
            .generic_arguments()
            .as_slice()
            .iter()
            .map(|argument| {
                self.resolver
                    .specialize_type(argument.ty(), substitution)
                    .map(|ty| GenericArgument::new(argument.parameter(), ty))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let arguments = GenericArguments::new(arguments)
            .map_err(|duplicate| ExecutableProgramError::DuplicateGeneric(duplicate.parameter()))?;
        Ok(DropSelection::new(selection.declaration(), arguments))
    }

    fn record_drop(
        &mut self,
        selection: DropSelection,
        drops: &mut BTreeMap<DropSelection, ExecutableItemKey>,
    ) -> Result<(), ExecutableProgramError> {
        if drops.contains_key(&selection) {
            return Ok(());
        }
        let key = ExecutableItemKey::Drop(DropInstanceKey::new_in(
            self.target,
            self.resolver.types(),
            selection.declaration(),
            selection.generic_arguments().clone(),
        )?);
        self.enqueue(key.clone());
        drops.insert(selection, key);
        Ok(())
    }
}

fn accepts_allocation_override(target: &TargetProgram, key: &ExecutableItemKey) -> bool {
    let ExecutableItemKey::Callable(key) = key else {
        return false;
    };
    target
        .checked()
        .graph()
        .declarations()
        .callables()
        .get(key.callable())
        .is_some_and(|callable| {
            matches!(
                callable.kind(),
                nocter_declarations::CallableKind::Literal(_)
            )
        })
}

struct ItemContext {
    body: BodyId,
    root: BodyNodeId,
}

fn item_context(
    target: &TargetProgram,
    key: &ExecutableItemKey,
) -> Result<ItemContext, ExecutableProgramError> {
    let checked = target.checked();
    let graph = checked.graph();
    let (body, root) = match key {
        ExecutableItemKey::Callable(key) => {
            let declaration = graph
                .declarations()
                .callables()
                .get(key.callable())
                .ok_or_else(|| {
                    ExecutableProgramError::UnknownItem(ExecutableItemKey::Callable(key.clone()))
                })?;
            let body = declaration
                .body()
                .ok_or(ExecutableProgramError::BodylessCallable(key.callable()))?;
            let root = checked
                .bodies()
                .get(body)
                .ok_or(ExecutableProgramError::UnknownBody(body))?
                .root();
            (body, root)
        }
        ExecutableItemKey::Closure(key) => {
            let definition = checked.closures().get(key.closure()).ok_or_else(|| {
                ExecutableProgramError::UnknownItem(ExecutableItemKey::Closure(key.clone()))
            })?;
            (definition.owner(), definition.body())
        }
        ExecutableItemKey::Drop(key) => {
            let declaration = graph
                .declarations()
                .drops()
                .get(key.drop())
                .ok_or_else(|| {
                    ExecutableProgramError::UnknownItem(ExecutableItemKey::Drop(key.clone()))
                })?;
            (
                declaration.body(),
                checked
                    .bodies()
                    .get(declaration.body())
                    .ok_or(ExecutableProgramError::UnknownBody(declaration.body()))?
                    .root(),
            )
        }
        ExecutableItemKey::Test(test) => {
            let declaration = graph
                .declarations()
                .tests()
                .get(*test)
                .ok_or_else(|| ExecutableProgramError::UnknownItem(key.clone()))?;
            (
                declaration.body(),
                checked
                    .bodies()
                    .get(declaration.body())
                    .ok_or(ExecutableProgramError::UnknownBody(declaration.body()))?
                    .root(),
            )
        }
    };
    Ok(ItemContext { body, root })
}

fn item_generic_arguments(key: &ExecutableItemKey) -> GenericArguments {
    match key {
        ExecutableItemKey::Callable(key) => key.generic_arguments().clone(),
        ExecutableItemKey::Closure(key) => key.generic_arguments().clone(),
        ExecutableItemKey::Drop(key) => key.generic_arguments().clone(),
        ExecutableItemKey::Test(_) => GenericArguments::default(),
    }
}

fn item_substitution(key: &ExecutableItemKey) -> TypeSubstitution {
    match key {
        ExecutableItemKey::Callable(key) => key.substitution(),
        ExecutableItemKey::Closure(key) => key.substitution(),
        ExecutableItemKey::Drop(key) => key.substitution(),
        ExecutableItemKey::Test(_) => TypeSubstitution::default(),
    }
}

fn collect_drops(plan: &ConcreteDestructionPlan, drops: &mut BTreeSet<DropSelection>) {
    match plan.kind() {
        ConcreteDestructionKind::Struct { drop, fields } => {
            drops.extend(drop.iter().cloned());
            for field in fields {
                collect_drops(field.plan(), drops);
            }
        }
        ConcreteDestructionKind::Enum { drop, variants } => {
            drops.extend(drop.iter().cloned());
            for variant in variants {
                for payload in variant.payload() {
                    collect_drops(payload.plan(), drops);
                }
            }
        }
        ConcreteDestructionKind::FixedArray { element, .. }
        | ConcreteDestructionKind::Optional(element) => collect_drops(element, drops),
        ConcreteDestructionKind::Fallible { success, failure } => {
            if let Some(success) = success {
                collect_drops(success, drops);
            }
            collect_drops(failure, drops);
        }
        ConcreteDestructionKind::Error => {}
        ConcreteDestructionKind::Closure(captures) => {
            for capture in captures {
                collect_drops(capture.plan(), drops);
            }
        }
        ConcreteDestructionKind::Opaque { plan, .. } => collect_drops(plan, drops),
    }
}

struct DraftItem {
    signature: super::ExecutableSignature,
    accepts_allocation_override: bool,
    closure: Option<super::ExecutableClosureLayout>,
    body: BodyId,
    root: BodyNodeId,
    nodes: Vec<BodyNodeId>,
    dispatches: Vec<DraftDispatchEdge>,
    closures: Vec<(nocter_model::ClosureId, ExecutableItemKey)>,
    drops: Vec<(DropSelection, ExecutableItemKey)>,
    types: Vec<ExecutableTypeEdge>,
    prepared_borrows: Vec<ExecutableBorrowEdge>,
    destructions: Vec<(CheckedDestruction, ConcreteDestructionPlan)>,
    sequences: Vec<sequence::DraftSequencePlan>,
    argument_packs: Vec<super::ExecutableArgumentPackPlan>,
}

struct DraftDispatchEdge {
    source: StaticSelection,
    plan: DraftDispatchPlan,
}

enum DraftDispatchPlan {
    Invocation(DraftDispatchStep),
    OpaqueInvocation {
        receiver: super::ExecutableOpaqueReceiver,
        operation: DraftDispatchStep,
    },
    Comparison {
        left_coercion: Option<DraftDispatchStep>,
        right_coercion: Option<DraftDispatchStep>,
        operation: DraftDispatchStep,
    },
    Index {
        receiver_coercion: Option<DraftDispatchStep>,
        operation: DraftDispatchStep,
    },
}

enum DraftDispatchStep {
    Direct(ExecutableItemKey),
    StandardPrimitive(ExecutablePrimitiveCall),
    StructuralPrimitive(nocter_checking::ResolvedPrimitiveDispatch),
    CallableValue(DraftCallableInvocation),
}

struct DraftCallableInvocation {
    subject: TypeId,
    contract: nocter_model::CallableContract,
    body: ExecutableItemKey,
    post_call_destruction: Option<ConcreteDestructionPlan>,
}

fn freeze_items(
    mut drafts: BTreeMap<ExecutableItemKey, DraftItem>,
    types: nocter_model::TypeStore,
    type_representations: super::RuntimeTypeRepresentationTable,
) -> Result<FrozenClosure, ExecutableProgramError> {
    let mut key_arena = ArenaBuilder::<ExecutableItemId, _>::new();
    let mut item_ids = BTreeMap::new();
    for key in drafts.keys() {
        let id = key_arena.insert(key.clone());
        item_ids.insert(key.clone(), id);
    }
    let items = key_arena.try_finish_with(|_item, key| {
        let draft = drafts
            .remove(&key)
            .ok_or_else(|| ExecutableProgramError::UnknownItem(key.clone()))?;
        let accepts_allocation_override = draft.accepts_allocation_override;
        let (signature, closure, body) = freeze_body(draft, &item_ids)?;
        Ok::<_, ExecutableProgramError>(ExecutableItem {
            key,
            signature,
            accepts_allocation_override,
            closure,
            body,
        })
    })?;
    Ok(FrozenClosure {
        types,
        items,
        item_ids,
        type_representations,
    })
}

fn freeze_body(
    draft: DraftItem,
    item_ids: &BTreeMap<ExecutableItemKey, ExecutableItemId>,
) -> Result<
    (
        super::ExecutableSignature,
        Option<super::ExecutableClosureLayout>,
        ExecutableBody,
    ),
    ExecutableProgramError,
> {
    let signature = draft.signature;
    let closure = draft.closure;
    let dispatches = draft
        .dispatches
        .into_iter()
        .map(|edge| {
            Ok(ExecutableDispatchEdge {
                source: edge.source,
                plan: freeze_dispatch_plan(edge.plan, item_ids)?,
            })
        })
        .collect::<Result<Vec<_>, ExecutableProgramError>>()?;
    let closures = draft
        .closures
        .into_iter()
        .map(|(closure, key)| {
            Ok(ExecutableClosureEdge {
                closure,
                item: item_id(item_ids, &key)?,
            })
        })
        .collect::<Result<Vec<_>, ExecutableProgramError>>()?;
    let drops = draft
        .drops
        .into_iter()
        .map(|(selection, key)| {
            Ok(ExecutableDropEdge {
                selection,
                item: item_id(item_ids, &key)?,
            })
        })
        .collect::<Result<Vec<_>, ExecutableProgramError>>()?;
    let sequences = draft
        .sequences
        .into_iter()
        .map(|plan| plan.freeze(item_ids))
        .collect::<Result<Vec<_>, ExecutableProgramError>>()?;
    Ok((
        signature,
        closure,
        ExecutableBody {
            body: draft.body,
            root: draft.root,
            nodes: draft.nodes.into_boxed_slice(),
            dispatches: dispatches.into_boxed_slice(),
            closures: closures.into_boxed_slice(),
            drops: drops.into_boxed_slice(),
            types: draft.types.into_boxed_slice(),
            prepared_borrows: draft.prepared_borrows.into_boxed_slice(),
            destructions: draft
                .destructions
                .into_iter()
                .map(|(source, plan)| ExecutableDestructionEdge { source, plan })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            sequences: sequences.into_boxed_slice(),
            argument_packs: draft.argument_packs.into_boxed_slice(),
        },
    ))
}

fn freeze_dispatch_step(
    step: DraftDispatchStep,
    item_ids: &BTreeMap<ExecutableItemKey, ExecutableItemId>,
) -> Result<ExecutableDispatchStep, ExecutableProgramError> {
    Ok(match step {
        DraftDispatchStep::Direct(key) => ExecutableDispatchStep::Direct(item_id(item_ids, &key)?),
        DraftDispatchStep::StandardPrimitive(call) => {
            ExecutableDispatchStep::StandardPrimitive(call)
        }
        DraftDispatchStep::StructuralPrimitive(primitive) => {
            ExecutableDispatchStep::StructuralPrimitive(primitive)
        }
        DraftDispatchStep::CallableValue(call) => {
            ExecutableDispatchStep::CallableValue(super::ExecutableCallableInvocation::new(
                call.subject,
                call.contract,
                item_id(item_ids, &call.body)?,
                call.post_call_destruction,
            ))
        }
    })
}

fn freeze_dispatch_plan(
    plan: DraftDispatchPlan,
    item_ids: &BTreeMap<ExecutableItemKey, ExecutableItemId>,
) -> Result<ExecutableDispatchPlan, ExecutableProgramError> {
    Ok(match plan {
        DraftDispatchPlan::Invocation(step) => {
            ExecutableDispatchPlan::Invocation(freeze_dispatch_step(step, item_ids)?)
        }
        DraftDispatchPlan::OpaqueInvocation {
            receiver,
            operation,
        } => ExecutableDispatchPlan::OpaqueInvocation {
            receiver,
            operation: freeze_dispatch_step(operation, item_ids)?,
        },
        DraftDispatchPlan::Comparison {
            left_coercion,
            right_coercion,
            operation,
        } => ExecutableDispatchPlan::Comparison {
            left_coercion: left_coercion
                .map(|step| freeze_dispatch_step(step, item_ids))
                .transpose()?,
            right_coercion: right_coercion
                .map(|step| freeze_dispatch_step(step, item_ids))
                .transpose()?,
            operation: freeze_dispatch_step(operation, item_ids)?,
        },
        DraftDispatchPlan::Index {
            receiver_coercion,
            operation,
        } => ExecutableDispatchPlan::Index {
            receiver_coercion: receiver_coercion
                .map(|step| freeze_dispatch_step(step, item_ids))
                .transpose()?,
            operation: freeze_dispatch_step(operation, item_ids)?,
        },
    })
}

fn item_id(
    item_ids: &BTreeMap<ExecutableItemKey, ExecutableItemId>,
    key: &ExecutableItemKey,
) -> Result<ExecutableItemId, ExecutableProgramError> {
    item_ids
        .get(key)
        .copied()
        .ok_or_else(|| ExecutableProgramError::UnknownItem(key.clone()))
}
