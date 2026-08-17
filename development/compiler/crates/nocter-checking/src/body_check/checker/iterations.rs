use nocter_declarations::{ExpansionCapability, StandardDeclarationRole};
use nocter_model::{BorrowCapability, CallableCapability, CallableId, TypeId, TypeKind};
use nocter_source_index::{SemanticEntity, SourceOrigin};
use nocter_syntax::{NodeId, NodeKind, Punctuation, TokenKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::copyability::Copyability;
use crate::instance_operations::{InstanceOperationSelector, MethodCandidate};
use crate::syntax::{direct_nodes, direct_token, is_transparent_expression};
use crate::{
    CheckedIteratorAcquisition, CheckedOperation, CheckedReceiver, IterationAcquisition,
    ReadonlyOperandPreparation, ReceiverPreparation, SpreadMode, StaticDispatch, StaticSelection,
    TypedIteration,
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
}

struct IteratorContract {
    iteration: TypedIteration,
    exact_size: StaticSelection,
}

impl BodyChecker<'_, '_> {
    /// Checks the acquisition and exact iteration contract for one sequence spread.
    ///
    /// Acquisition has its own checked node because it owns the iterator temporary independently
    /// of the source expression. Direct owned iteration is selected before owned expansion and is
    /// never allowed to fall back when the direct iterator lacks exact-size support.
    pub(super) fn check_sequence_spread(
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
        let contract = self.select_iterator_contract(element, &acquired)?;
        let item = contract.iteration.item();
        let Some(contribution) = mode.contribution_type(self.types, item) else {
            return Err(self.rule(BodyRule::InvalidSpreadElement, spread)?);
        };
        if mode == SpreadMode::Copy
            && self
                .copyabilities
                .classify(self.graph, self.types, contribution)
                .map_err(BodyCheckInternalError::Copyability)?
                != Copyability::Copy
        {
            return Err(self.rule(BodyRule::InvalidSpreadElement, spread)?);
        }
        Ok(CheckedSpreadDraft {
            mode,
            iteration: contract.iteration,
            exact_size: contract.exact_size,
            contribution,
        })
    }

    fn acquire_readonly_spread(
        &mut self,
        element: NodeId,
        source: NodeId,
    ) -> Result<AcquiredIterator, BodyCheckError> {
        let operand = self.check_readonly_operand(source, None)?;
        let candidates = {
            let mut selector = self.operation_selector();
            selector
                .select_expansions(operand.owner, ExpansionCapability::Readonly)
                .map_err(BodyCheckInternalError::from)?
        };
        let [candidate] = candidates.as_slice() else {
            return Err(self.rule(BodyRule::InvalidSpreadAcquisition, element)?);
        };
        let receiver = CheckedReceiver::new(
            operand.value,
            match operand.preparation {
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
            },
            None,
        );
        let selection = candidate.selection().clone();
        self.project_expansion(element, selection.dispatch())?;
        let ty = candidate.result();
        let node = self.add_node(
            element,
            ty,
            CheckedOperation::IteratorAcquisition(CheckedIteratorAcquisition::new(
                receiver,
                IterationAcquisition::Expansion(selection),
            )),
        )?;
        Ok(AcquiredIterator { node, ty })
    }

    fn acquire_owned_spread(
        &mut self,
        element: NodeId,
        source: NodeId,
    ) -> Result<AcquiredIterator, BodyCheckError> {
        let value = self.check_expression(source, None)?;
        let source_type = self.node_type(value)?;
        let iterator_methods = self.select_iterator_methods(source_type)?;
        if iterator_methods.len() > 1 {
            return Err(self.rule(BodyRule::InvalidSpreadIterator, element)?);
        }
        if iterator_methods.len() == 1 {
            let receiver = CheckedReceiver::new(value, ReceiverPreparation::Owned, None);
            let node = self.add_node(
                element,
                source_type,
                CheckedOperation::IteratorAcquisition(CheckedIteratorAcquisition::new(
                    receiver,
                    IterationAcquisition::Direct,
                )),
            )?;
            return Ok(AcquiredIterator {
                node,
                ty: source_type,
            });
        }

        let candidates = {
            let mut selector = self.operation_selector();
            selector
                .select_expansions(source_type, ExpansionCapability::Owned)
                .map_err(BodyCheckInternalError::from)?
        };
        let [candidate] = candidates.as_slice() else {
            return Err(self.rule(BodyRule::InvalidSpreadAcquisition, element)?);
        };
        let selection = candidate.selection().clone();
        self.project_expansion(element, selection.dispatch())?;
        let ty = candidate.result();
        let node = self.add_node(
            element,
            ty,
            CheckedOperation::IteratorAcquisition(CheckedIteratorAcquisition::new(
                CheckedReceiver::new(value, ReceiverPreparation::Owned, None),
                IterationAcquisition::Expansion(selection),
            )),
        )?;
        Ok(AcquiredIterator { node, ty })
    }

    fn select_iterator_contract(
        &mut self,
        element: NodeId,
        acquired: &AcquiredIterator,
    ) -> Result<IteratorContract, BodyCheckError> {
        let iterator_methods = self.select_iterator_methods(acquired.ty)?;
        let [next] = iterator_methods.as_slice() else {
            return Err(self.rule(BodyRule::InvalidSpreadIterator, element)?);
        };
        if next.receiver_capability() != CallableCapability::ReadWrite {
            return Err(BodyCheckInternalError::MissingIterationSemanticRoles.into());
        }
        let callable = self
            .graph
            .declarations()
            .callables()
            .get(next.callable())
            .ok_or(BodyCheckInternalError::MissingCallable(next.callable()))?;
        let result = next
            .substitution()
            .apply_type(self.types, callable.result())
            .map_err(BodyCheckInternalError::CallSubstitution)?;
        let Some(TypeKind::Optional(item)) = self.types.get(result) else {
            return Err(BodyCheckInternalError::MissingIterationSemanticRoles.into());
        };
        let item = *item;

        let (exact_interface, exact_method) = self.exact_size_roles()?;
        let exact = {
            let mut selector = self.operation_selector();
            selector
                .select_exact_interface_method(acquired.ty, exact_interface, exact_method)
                .map_err(BodyCheckInternalError::from)?
        };
        let [exact] = exact.as_slice() else {
            return Err(self.rule(BodyRule::InvalidSpreadIterator, element)?);
        };
        if exact.receiver_capability() != CallableCapability::Readonly {
            return Err(BodyCheckInternalError::MissingIterationSemanticRoles.into());
        }
        Ok(IteratorContract {
            iteration: TypedIteration::new(acquired.node, method_selection(next), item),
            exact_size: method_selection(exact),
        })
    }

    fn select_iterator_methods(
        &mut self,
        target: TypeId,
    ) -> Result<Vec<MethodCandidate>, BodyCheckError> {
        let (interface, method) = self.iterator_roles()?;
        let selected = {
            let mut selector = self.operation_selector();
            selector
                .select_exact_interface_method(target, interface, method)
                .map_err(BodyCheckInternalError::from)?
        };
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

    fn operation_selector(&mut self) -> InstanceOperationSelector<'_> {
        InstanceOperationSelector::new(
            self.graph,
            self.types,
            self.conformances,
            self.copyabilities,
            self.instance_operations,
            &self.assumptions,
            self.source.module(),
        )
    }

    fn project_expansion(
        &mut self,
        element: NodeId,
        dispatch: StaticDispatch,
    ) -> Result<(), BodyCheckInternalError> {
        let token = direct_token(self.tree(), element)
            .filter(|token| token.kind() == TokenKind::Punctuation(Punctuation::Expansion))
            .ok_or(BodyCheckInternalError::InvalidSyntax(element))?;
        let entity = match dispatch {
            StaticDispatch::Direct(callable) => SemanticEntity::Callable(callable),
            StaticDispatch::StructuralRequirement(requirement) => {
                SemanticEntity::Requirement(requirement)
            }
            StaticDispatch::InterfaceMethod { .. } => {
                return Err(BodyCheckInternalError::CallContractSelection);
            }
        };
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(element))?;
        self.projections
            .push(super::NodeProjection { entity, origin });
        Ok(())
    }
}

fn method_selection(candidate: &MethodCandidate) -> StaticSelection {
    StaticSelection::new(candidate.dispatch(), candidate.generic_arguments().clone())
}

fn spread_mode(checker: &BodyChecker<'_, '_>, root: NodeId) -> Result<SpreadMode, BodyCheckError> {
    let mut syntax = root;
    while checker.kind(syntax).is_ok_and(is_transparent_expression) {
        let children = direct_nodes(checker.tree(), syntax);
        let [child] = children.as_slice() else {
            break;
        };
        syntax = *child;
    }
    match checker.kind(syntax)? {
        NodeKind::MoveExpression => Ok(SpreadMode::Move),
        NodeKind::UnaryExpression => {
            match direct_token(checker.tree(), syntax).map(nocter_syntax::SyntaxToken::kind) {
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
