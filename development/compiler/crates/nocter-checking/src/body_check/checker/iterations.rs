use nocter_declarations::ExpansionCapability;
use nocter_model::{BorrowCapability, CallableCapability, CallableId, TypeId, TypeKind};
use nocter_source_index::{SemanticEntity, SourceOrigin};
use nocter_syntax::{Keyword, NodeId, NodeKind, Punctuation, TokenKind};
use nocter_toolchain_contract::StandardDeclarationRole;

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::copyability::Copyability;
use crate::instance_operations::MethodCandidate;
use crate::syntax::{child_nodes, first_direct_token, is_transparent_expression};
use crate::{
    CheckedIteratorAcquisition, CheckedOperation, CheckedReceiver, IterationAcquisition,
    IterationItemOrigin, OutcomeLayer, ReadonlyOperandPreparation, ReceiverPreparation, SpreadMode,
    StaticDispatch, StaticSelection, TypedAsyncIteration, TypedIteration,
};

pub(super) struct CheckedSpreadDraft {
    pub(super) mode: SpreadMode,
    pub(super) iteration: TypedIteration,
    pub(super) exact_size: StaticSelection,
    pub(super) contribution: TypeId,
}

struct AcquiredIterator {
    node: nocter_model::BodyNodeId,
    ty: TypeId,
    next: Option<SelectedIteratorMethod>,
}

struct SelectedIteratorMethod {
    method: MethodCandidate,
    item_origin: IterationItemOrigin,
}

#[derive(Clone, Copy)]
struct IterationRules {
    acquisition: BodyRule,
    iterator: BodyRule,
}

#[derive(Clone, Copy)]
struct ExpansionProjection {
    syntax: NodeId,
    token: TokenKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectionSourceMode {
    Readonly(NodeId),
    ReadWrite(NodeId),
    Move(NodeId),
    Bare,
}

const SPREAD_RULES: IterationRules = IterationRules {
    acquisition: BodyRule::InvalidSpreadAcquisition,
    iterator: BodyRule::InvalidSpreadIterator,
};

const COLLECTION_RULES: IterationRules = IterationRules {
    acquisition: BodyRule::InvalidCollectionAcquisition,
    iterator: BodyRule::InvalidCollectionIterator,
};

const ASYNC_COLLECTION_RULES: IterationRules = IterationRules {
    acquisition: BodyRule::InvalidAsyncCollectionIterator,
    iterator: BodyRule::InvalidAsyncCollectionIterator,
};

impl BodyChecker<'_, '_> {
    /// Checks the acquisition and exact iteration contract for one argument-pack spread.
    ///
    /// Acquisition has its own checked node because it owns the iterator temporary independently
    /// of the source expression. Direct owned iteration is selected before owned expansion and is
    /// never allowed to fall back when the direct iterator lacks exact-size support.
    pub(super) fn check_argument_spread(
        &mut self,
        element: NodeId,
        spread: NodeId,
    ) -> Result<CheckedSpreadDraft, BodyCheckError> {
        let source = self.required_child(spread, NodeKind::Expression)?;
        let mode = spread_mode(self, source)?;
        let acquired = match mode {
            SpreadMode::Copy | SpreadMode::Borrow => {
                self.acquire_readonly_spread(element, source)?
            }
            SpreadMode::Move => self.acquire_owned_spread(element, source)?,
        };
        let iterator_type = acquired.ty;
        let iteration = self.select_typed_iteration(element, acquired, SPREAD_RULES, false)?;
        let exact_size = self.select_exact_size(element, iterator_type, SPREAD_RULES)?;
        let item = iteration.item();
        let Some(contribution) = mode.contribution_type(self.types, item) else {
            return Err(self.rule(BodyRule::InvalidSpreadElement, spread)?);
        };
        if mode == SpreadMode::Copy && self.classify_copyability(contribution)? != Copyability::Copy
        {
            return Err(self.rule(BodyRule::InvalidSpreadElement, spread)?);
        }
        Ok(CheckedSpreadDraft {
            mode,
            iteration,
            exact_size,
            contribution,
        })
    }

    /// Checks one collection-loop source and freezes its acquisition and `Iterator.next` plan.
    ///
    /// Explicit borrows select only their matching expansion capability. An explicit move gives
    /// direct iterator interface implementation fixed priority over owned expansion. A bare expression is
    /// accepted only as a direct iterator and keeps ordinary copy/move checking.
    pub(super) fn check_collection_iteration(
        &mut self,
        owner: NodeId,
        source: NodeId,
    ) -> Result<TypedIteration, BodyCheckError> {
        let acquired = match collection_source_mode(self, source)? {
            CollectionSourceMode::Readonly(modifier) => {
                self.acquire_readonly_collection(owner, modifier)?
            }
            CollectionSourceMode::ReadWrite(modifier) => {
                self.acquire_readwrite_collection(owner, modifier)?
            }
            CollectionSourceMode::Move(modifier) => self.acquire_owned_iteration(
                owner,
                source,
                ExpansionProjection {
                    syntax: modifier,
                    token: TokenKind::Keyword(Keyword::Move),
                },
                COLLECTION_RULES,
            )?,
            CollectionSourceMode::Bare => {
                self.acquire_direct_iteration(owner, source, COLLECTION_RULES)?
            }
        };
        self.select_typed_iteration(owner, acquired, COLLECTION_RULES, true)
    }

    /// Checks one owned asynchronous iterator and freezes its exact `AsyncIterator.next` plan.
    ///
    /// Async iteration intentionally has no expansion fallback. The source must itself implement
    /// the asynchronous protocol, and ordinary expression checking remains the sole authority for
    /// whether an explicit `move` is required.
    pub(super) fn check_async_collection_iteration(
        &mut self,
        owner: NodeId,
        source: NodeId,
        failure_outer: impl Into<Box<[OutcomeLayer]>>,
    ) -> Result<TypedAsyncIteration, BodyCheckError> {
        let value = self.check_expression(source, None)?;
        let ty = self.node_type(value)?;
        let mut methods = self.select_async_collection_iterator_methods(ty)?;
        if methods.len() != 1 {
            return Err(self.rule(ASYNC_COLLECTION_RULES.acquisition, owner)?);
        }
        let next = methods.remove(0);
        if next.method.receiver_capability() != CallableCapability::ReadWrite {
            return Err(BodyCheckInternalError::MissingIterationSemanticRoles.into());
        }
        let callable = self
            .graph
            .declarations()
            .callables()
            .get(next.method.callable())
            .ok_or(BodyCheckInternalError::MissingCallable(
                next.method.callable(),
            ))?;
        let result = self.apply_type_substitution(next.method.substitution(), callable.result())?;
        let Some(TypeKind::Future(deferred)) = self.types.get(result) else {
            return Err(BodyCheckInternalError::MissingIterationSemanticRoles.into());
        };
        let Some(TypeKind::Fallible(optional)) = self.types.get(*deferred) else {
            return Err(BodyCheckInternalError::MissingIterationSemanticRoles.into());
        };
        let Some(TypeKind::Optional(item)) = self.types.get(*optional) else {
            return Err(BodyCheckInternalError::MissingIterationSemanticRoles.into());
        };
        let item = *item;
        let iterator = self.add_node(
            owner,
            ty,
            CheckedOperation::IteratorAcquisition(CheckedIteratorAcquisition::new(
                CheckedReceiver::new(value, ReceiverPreparation::Owned, None),
                IterationAcquisition::Direct,
            )),
        )?;
        Ok(TypedAsyncIteration::new(
            iterator,
            method_selection(&next.method),
            item,
            next.item_origin,
            failure_outer,
        ))
    }

    fn acquire_readonly_spread(
        &mut self,
        element: NodeId,
        source: NodeId,
    ) -> Result<AcquiredIterator, BodyCheckError> {
        let operand = self.check_readonly_operand(source, None)?;
        let receiver = CheckedReceiver::new(
            operand.value,
            readonly_receiver_preparation(operand.preparation),
            None,
        );
        self.acquire_selected_expansion(
            element,
            receiver,
            operand.owner,
            ExpansionCapability::Readonly,
            ExpansionProjection {
                syntax: element,
                token: TokenKind::Punctuation(Punctuation::Expansion),
            },
            SPREAD_RULES,
        )
    }

    fn acquire_owned_spread(
        &mut self,
        element: NodeId,
        source: NodeId,
    ) -> Result<AcquiredIterator, BodyCheckError> {
        self.acquire_owned_iteration(
            element,
            source,
            ExpansionProjection {
                syntax: element,
                token: TokenKind::Punctuation(Punctuation::Expansion),
            },
            SPREAD_RULES,
        )
    }

    fn acquire_readonly_collection(
        &mut self,
        owner: NodeId,
        modifier: NodeId,
    ) -> Result<AcquiredIterator, BodyCheckError> {
        let operand_syntax = modifier_operand(self, modifier)?;
        let operand = self.check_readonly_operand(operand_syntax, None)?;
        self.acquire_selected_expansion(
            owner,
            CheckedReceiver::new(
                operand.value,
                readonly_receiver_preparation(operand.preparation),
                None,
            ),
            operand.owner,
            ExpansionCapability::Readonly,
            ExpansionProjection {
                syntax: modifier,
                token: TokenKind::Punctuation(Punctuation::Ampersand),
            },
            COLLECTION_RULES,
        )
    }

    fn acquire_readwrite_collection(
        &mut self,
        owner: NodeId,
        modifier: NodeId,
    ) -> Result<AcquiredIterator, BodyCheckError> {
        let operand_syntax = modifier_operand(self, modifier)?;
        if self.is_constant_reference(operand_syntax) {
            return Err(self.rule(BodyRule::InvalidReadWriteBorrow, operand_syntax)?);
        }
        let place = self.postfix_place(operand_syntax, BorrowCapability::ReadWrite)?;
        if !self.is_writable_place(place.id)? {
            return Err(self.rule(BodyRule::InvalidReadWriteBorrow, operand_syntax)?);
        }
        let value = self.add_node(operand_syntax, place.ty, CheckedOperation::Place(place.id))?;
        self.acquire_selected_expansion(
            owner,
            CheckedReceiver::new(
                value,
                ReceiverPreparation::BorrowPlace(BorrowCapability::ReadWrite),
                None,
            ),
            place.ty,
            ExpansionCapability::ReadWrite,
            ExpansionProjection {
                syntax: modifier,
                token: TokenKind::Punctuation(Punctuation::ReadWrite),
            },
            COLLECTION_RULES,
        )
    }

    fn acquire_selected_expansion(
        &mut self,
        owner: NodeId,
        receiver: CheckedReceiver,
        source_type: TypeId,
        capability: ExpansionCapability,
        projection: ExpansionProjection,
        rules: IterationRules,
    ) -> Result<AcquiredIterator, BodyCheckError> {
        let candidates = {
            let mut selector = self.instance_selector();
            selector
                .select_expansions(source_type, capability)
                .map_err(BodyCheckInternalError::from)?
        };
        let [candidate] = candidates.as_slice() else {
            return Err(self.rule(rules.acquisition, owner)?);
        };
        let selection = candidate.selection().clone();
        self.project_expansion(projection.syntax, projection.token, selection.dispatch())?;
        let ty = candidate.result();
        let node = self.add_node(
            owner,
            ty,
            CheckedOperation::IteratorAcquisition(CheckedIteratorAcquisition::new(
                receiver,
                IterationAcquisition::Expansion(selection),
            )),
        )?;
        Ok(AcquiredIterator {
            node,
            ty,
            next: None,
        })
    }

    fn acquire_owned_iteration(
        &mut self,
        owner: NodeId,
        source: NodeId,
        expansion_projection: ExpansionProjection,
        rules: IterationRules,
    ) -> Result<AcquiredIterator, BodyCheckError> {
        let value = self.check_expression(source, None)?;
        let source_type = self.node_type(value)?;
        let mut iterator_methods = self.select_collection_iterator_methods(source_type)?;
        if iterator_methods.len() > 1 {
            return Err(self.rule(rules.iterator, owner)?);
        }
        if iterator_methods.len() == 1 {
            let next = iterator_methods.remove(0);
            let receiver = CheckedReceiver::new(value, ReceiverPreparation::Owned, None);
            let node = self.add_node(
                owner,
                source_type,
                CheckedOperation::IteratorAcquisition(CheckedIteratorAcquisition::new(
                    receiver,
                    IterationAcquisition::Direct,
                )),
            )?;
            return Ok(AcquiredIterator {
                node,
                ty: source_type,
                next: Some(next),
            });
        }

        let candidates = {
            let mut selector = self.instance_selector();
            selector
                .select_expansions(source_type, ExpansionCapability::Owned)
                .map_err(BodyCheckInternalError::from)?
        };
        let [candidate] = candidates.as_slice() else {
            return Err(self.rule(rules.acquisition, owner)?);
        };
        let selection = candidate.selection().clone();
        self.project_expansion(
            expansion_projection.syntax,
            expansion_projection.token,
            selection.dispatch(),
        )?;
        let ty = candidate.result();
        let node = self.add_node(
            owner,
            ty,
            CheckedOperation::IteratorAcquisition(CheckedIteratorAcquisition::new(
                CheckedReceiver::new(value, ReceiverPreparation::Owned, None),
                IterationAcquisition::Expansion(selection),
            )),
        )?;
        Ok(AcquiredIterator {
            node,
            ty,
            next: None,
        })
    }

    fn acquire_direct_iteration(
        &mut self,
        owner: NodeId,
        source: NodeId,
        rules: IterationRules,
    ) -> Result<AcquiredIterator, BodyCheckError> {
        let value = self.check_expression(source, None)?;
        let ty = self.node_type(value)?;
        let mut methods = self.select_collection_iterator_methods(ty)?;
        if methods.len() != 1 {
            return Err(self.rule(rules.acquisition, owner)?);
        }
        let next = methods.remove(0);
        let node = self.add_node(
            owner,
            ty,
            CheckedOperation::IteratorAcquisition(CheckedIteratorAcquisition::new(
                CheckedReceiver::new(value, ReceiverPreparation::Owned, None),
                IterationAcquisition::Direct,
            )),
        )?;
        Ok(AcquiredIterator {
            node,
            ty,
            next: Some(next),
        })
    }

    fn select_typed_iteration(
        &mut self,
        owner: NodeId,
        acquired: AcquiredIterator,
        rules: IterationRules,
        allow_lending: bool,
    ) -> Result<TypedIteration, BodyCheckError> {
        let next = if let Some(next) = acquired.next {
            next
        } else {
            let mut iterator_methods = if allow_lending {
                self.select_collection_iterator_methods(acquired.ty)?
            } else {
                self.select_iterator_methods(acquired.ty)?
                    .into_iter()
                    .map(|method| SelectedIteratorMethod {
                        method,
                        item_origin: IterationItemOrigin::CallableResult,
                    })
                    .collect()
            };
            if iterator_methods.len() != 1 {
                return Err(self.rule(rules.iterator, owner)?);
            }
            iterator_methods.remove(0)
        };
        if next.method.receiver_capability() != CallableCapability::ReadWrite {
            return Err(BodyCheckInternalError::MissingIterationSemanticRoles.into());
        }
        let callable = self
            .graph
            .declarations()
            .callables()
            .get(next.method.callable())
            .ok_or(BodyCheckInternalError::MissingCallable(
                next.method.callable(),
            ))?;
        let result = self.apply_type_substitution(next.method.substitution(), callable.result())?;
        let Some(TypeKind::Optional(item)) = self.types.get(result) else {
            return Err(BodyCheckInternalError::MissingIterationSemanticRoles.into());
        };
        let item = *item;

        Ok(TypedIteration::new(
            acquired.node,
            method_selection(&next.method),
            item,
            next.item_origin,
        ))
    }

    fn select_exact_size(
        &mut self,
        owner: NodeId,
        iterator: TypeId,
        rules: IterationRules,
    ) -> Result<StaticSelection, BodyCheckError> {
        let (exact_interface, exact_method) = self.exact_size_roles()?;
        let exact = {
            let mut selector = self.instance_selector();
            selector
                .select_exact_interface_method(iterator, exact_interface, exact_method)
                .map_err(BodyCheckInternalError::from)?
        };
        let [exact] = exact.as_slice() else {
            return Err(self.rule(rules.iterator, owner)?);
        };
        if exact.receiver_capability() != CallableCapability::Readonly {
            return Err(BodyCheckInternalError::MissingIterationSemanticRoles.into());
        }
        Ok(method_selection(exact))
    }

    fn select_iterator_methods(
        &mut self,
        target: TypeId,
    ) -> Result<Vec<MethodCandidate>, BodyCheckError> {
        let (interface, method) = self.iterator_roles()?;
        let selected = {
            let mut selector = self.instance_selector();
            selector
                .select_exact_interface_method(target, interface, method)
                .map_err(BodyCheckInternalError::from)?
        };
        Ok(selected)
    }

    fn select_lending_iterator_methods(
        &mut self,
        target: TypeId,
    ) -> Result<Vec<MethodCandidate>, BodyCheckError> {
        let Some((interface, method)) = self.lending_iterator_roles()? else {
            return Ok(Vec::new());
        };
        let selected = {
            let mut selector = self.instance_selector();
            selector
                .select_exact_interface_method(target, interface, method)
                .map_err(BodyCheckInternalError::from)?
        };
        Ok(selected)
    }

    fn select_collection_iterator_methods(
        &mut self,
        target: TypeId,
    ) -> Result<Vec<SelectedIteratorMethod>, BodyCheckError> {
        let mut selected = self
            .select_iterator_methods(target)?
            .into_iter()
            .map(|method| SelectedIteratorMethod {
                method,
                item_origin: IterationItemOrigin::CallableResult,
            })
            .collect::<Vec<_>>();
        selected.extend(
            self.select_lending_iterator_methods(target)?
                .into_iter()
                .map(|method| SelectedIteratorMethod {
                    method,
                    item_origin: IterationItemOrigin::ReceiverLoan,
                }),
        );
        Ok(selected)
    }

    fn select_async_iterator_methods(
        &mut self,
        target: TypeId,
    ) -> Result<Vec<MethodCandidate>, BodyCheckError> {
        let (interface, method) = self.async_iterator_roles()?;
        let selected = {
            let mut selector = self.instance_selector();
            selector
                .select_exact_interface_method(target, interface, method)
                .map_err(BodyCheckInternalError::from)?
        };
        Ok(selected)
    }

    fn select_async_lending_iterator_methods(
        &mut self,
        target: TypeId,
    ) -> Result<Vec<MethodCandidate>, BodyCheckError> {
        let Some((interface, method)) = self.async_lending_iterator_roles()? else {
            return Ok(Vec::new());
        };
        let selected = {
            let mut selector = self.instance_selector();
            selector
                .select_exact_interface_method(target, interface, method)
                .map_err(BodyCheckInternalError::from)?
        };
        Ok(selected)
    }

    fn select_async_collection_iterator_methods(
        &mut self,
        target: TypeId,
    ) -> Result<Vec<SelectedIteratorMethod>, BodyCheckError> {
        let mut selected = self
            .select_async_iterator_methods(target)?
            .into_iter()
            .map(|method| SelectedIteratorMethod {
                method,
                item_origin: IterationItemOrigin::CallableResult,
            })
            .collect::<Vec<_>>();
        selected.extend(
            self.select_async_lending_iterator_methods(target)?
                .into_iter()
                .map(|method| SelectedIteratorMethod {
                    method,
                    item_origin: IterationItemOrigin::ReceiverLoan,
                }),
        );
        Ok(selected)
    }

    fn iterator_roles(&self) -> Result<(nocter_model::InterfaceId, CallableId), BodyCheckError> {
        match (
            self.standard_semantics
                .interface(StandardDeclarationRole::IteratorInterface),
            self.standard_semantics
                .callable(StandardDeclarationRole::IteratorNextMethod),
        ) {
            (Some(interface), Some(method)) => Ok((interface, method)),
            _ => Err(BodyCheckInternalError::MissingIterationSemanticRoles.into()),
        }
    }

    fn exact_size_roles(&self) -> Result<(nocter_model::InterfaceId, CallableId), BodyCheckError> {
        match (
            self.standard_semantics
                .interface(StandardDeclarationRole::ExactSizeIteratorInterface),
            self.standard_semantics
                .callable(StandardDeclarationRole::ExactSizeIteratorRemainingLenMethod),
        ) {
            (Some(interface), Some(method)) => Ok((interface, method)),
            _ => Err(BodyCheckInternalError::MissingIterationSemanticRoles.into()),
        }
    }

    fn lending_iterator_roles(
        &self,
    ) -> Result<Option<(nocter_model::InterfaceId, CallableId)>, BodyCheckError> {
        match (
            self.standard_semantics
                .interface(StandardDeclarationRole::LendingIteratorInterface),
            self.standard_semantics
                .callable(StandardDeclarationRole::LendingIteratorNextMethod),
        ) {
            (Some(interface), Some(method)) => Ok(Some((interface, method))),
            (None, None) => Ok(None),
            _ => Err(BodyCheckInternalError::MissingIterationSemanticRoles.into()),
        }
    }

    fn async_iterator_roles(
        &self,
    ) -> Result<(nocter_model::InterfaceId, CallableId), BodyCheckError> {
        match (
            self.standard_semantics
                .interface(StandardDeclarationRole::AsyncIteratorInterface),
            self.standard_semantics
                .callable(StandardDeclarationRole::AsyncIteratorNextMethod),
        ) {
            (Some(interface), Some(method)) => Ok((interface, method)),
            _ => Err(BodyCheckInternalError::MissingIterationSemanticRoles.into()),
        }
    }

    fn async_lending_iterator_roles(
        &self,
    ) -> Result<Option<(nocter_model::InterfaceId, CallableId)>, BodyCheckError> {
        match (
            self.standard_semantics
                .interface(StandardDeclarationRole::AsyncLendingIteratorInterface),
            self.standard_semantics
                .callable(StandardDeclarationRole::AsyncLendingIteratorNextMethod),
        ) {
            (Some(interface), Some(method)) => Ok(Some((interface, method))),
            (None, None) => Ok(None),
            _ => Err(BodyCheckInternalError::MissingIterationSemanticRoles.into()),
        }
    }

    fn project_expansion(
        &mut self,
        syntax: NodeId,
        token_kind: TokenKind,
        dispatch: StaticDispatch,
    ) -> Result<(), BodyCheckInternalError> {
        let token = first_direct_token(self.tree(), syntax)
            .filter(|token| token.kind() == token_kind)
            .ok_or(BodyCheckInternalError::InvalidSyntax(syntax))?;
        let entity = match dispatch {
            StaticDispatch::Direct(callable)
            | StaticDispatch::InterfaceDefault {
                method: callable, ..
            } => SemanticEntity::Callable(callable),
            StaticDispatch::StructuralRequirement { evidence } => {
                if self.capability_evidence.get(evidence).is_none() {
                    return Err(BodyCheckInternalError::CallContractSelection);
                }
                SemanticEntity::CapabilityEvidence(evidence)
            }
            StaticDispatch::InterfaceMethod { .. }
            | StaticDispatch::InterfaceSelfMethod { .. }
            | StaticDispatch::OpaqueMethod { .. } => {
                return Err(BodyCheckInternalError::CallContractSelection);
            }
        };
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(syntax))?;
        self.projections
            .push(super::NodeProjection::new(entity, origin));
        Ok(())
    }
}

fn method_selection(candidate: &MethodCandidate) -> StaticSelection {
    StaticSelection::new(candidate.dispatch(), candidate.generic_arguments().clone())
}

const fn readonly_receiver_preparation(
    preparation: ReadonlyOperandPreparation,
) -> ReceiverPreparation {
    match preparation {
        ReadonlyOperandPreparation::BorrowPlace => {
            ReceiverPreparation::BorrowPlace(BorrowCapability::Readonly)
        }
        ReadonlyOperandPreparation::BorrowTemporary => {
            ReceiverPreparation::BorrowTemporary(BorrowCapability::Readonly)
        }
        ReadonlyOperandPreparation::UseReadonlyBorrow => {
            ReceiverPreparation::PreserveBorrow(BorrowCapability::Readonly)
        }
        ReadonlyOperandPreparation::WeakenReadwriteBorrow => {
            ReceiverPreparation::WeakenReadwriteBorrow
        }
    }
}

fn modifier_operand(
    checker: &BodyChecker<'_, '_>,
    modifier: NodeId,
) -> Result<NodeId, BodyCheckError> {
    let children = child_nodes(checker.tree(), modifier);
    let [operand] = children.as_slice() else {
        return Err(BodyCheckInternalError::InvalidSyntax(modifier).into());
    };
    Ok(*operand)
}

fn spread_mode(checker: &BodyChecker<'_, '_>, root: NodeId) -> Result<SpreadMode, BodyCheckError> {
    let mut syntax = root;
    while checker.kind(syntax).is_ok_and(is_transparent_expression) {
        let children = child_nodes(checker.tree(), syntax);
        let [child] = children.as_slice() else {
            break;
        };
        syntax = *child;
    }
    match checker.kind(syntax)? {
        NodeKind::MoveExpression => Ok(SpreadMode::Move),
        NodeKind::UnaryExpression => {
            match first_direct_token(checker.tree(), syntax).map(nocter_syntax::SyntaxToken::kind) {
                Some(TokenKind::Punctuation(Punctuation::Ampersand)) => Ok(SpreadMode::Borrow),
                Some(TokenKind::Punctuation(Punctuation::ReadWrite)) => {
                    Err(checker.rule(BodyRule::InvalidSpreadAcquisition, syntax)?)
                }
                _ => Ok(SpreadMode::Copy),
            }
        }
        _ => Ok(SpreadMode::Copy),
    }
}

fn collection_source_mode(
    checker: &BodyChecker<'_, '_>,
    root: NodeId,
) -> Result<CollectionSourceMode, BodyCheckError> {
    let mut syntax = root;
    while checker.kind(syntax).is_ok_and(is_transparent_expression) {
        let children = child_nodes(checker.tree(), syntax);
        let [child] = children.as_slice() else {
            break;
        };
        syntax = *child;
    }
    match checker.kind(syntax)? {
        NodeKind::MoveExpression => Ok(CollectionSourceMode::Move(syntax)),
        NodeKind::UnaryExpression => {
            match first_direct_token(checker.tree(), syntax).map(nocter_syntax::SyntaxToken::kind) {
                Some(TokenKind::Punctuation(Punctuation::Ampersand)) => {
                    Ok(CollectionSourceMode::Readonly(syntax))
                }
                Some(TokenKind::Punctuation(Punctuation::ReadWrite)) => {
                    Ok(CollectionSourceMode::ReadWrite(syntax))
                }
                _ => Ok(CollectionSourceMode::Bare),
            }
        }
        _ => Ok(CollectionSourceMode::Bare),
    }
}
