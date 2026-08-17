use std::collections::HashSet;

use nocter_declarations::{ExportedEntity, NominalShape, ParameterOwner, ParameterRole};
use nocter_model::{
    BodyNodeId, BorrowCapability, BuiltinType, NominalTypeId, TypeId, TypeKind, VariantId,
};
use nocter_source_index::{SemanticEntity, SyntaxOrigin};
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, TokenKind};

use super::{BlockExpectation, BodyChecker};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::copyability::Copyability;
use crate::syntax::{
    descendants, direct_child, direct_children, direct_identifier, direct_nodes, identifier_tokens,
    is_transparent_expression,
};
use crate::type_relations::TypeSubstitution;
use crate::{
    CheckedControl, CheckedOperation, CheckedPattern, CheckedPatternArm, CheckedPatternFallback,
    CheckedPatternSlot, CheckedPatternSubject, PatternSubjectPreparation,
};

struct PatternSubjectPlan {
    checked: CheckedPatternSubject,
    arguments: Box<[TypeId]>,
}

struct ResolvedPatternVariant {
    nominal: NominalTypeId,
    variant: VariantId,
    parameters: Vec<nocter_model::ParameterId>,
    substitution: TypeSubstitution,
}

impl BodyChecker<'_, '_> {
    pub(super) fn check_pattern_if(
        &mut self,
        node: NodeId,
        condition: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let subject_syntax = self.required_child(condition, NodeKind::Expression)?;
        let subject = self.check_pattern_subject(subject_syntax)?;
        let pattern_syntax = self.required_child(condition, NodeKind::EnumPattern)?;
        let pattern = self.check_enum_pattern(pattern_syntax, &subject)?;

        let else_syntax = direct_child(self.tree(), node, NodeKind::ElseClause);
        let branches = self.check_if_branches(node, expected)?;
        let fallback = branches
            .else_branch
            .map(|body| CheckedPatternFallback::new(body, true));
        let checked = self.add_node(
            node,
            branches.ty,
            CheckedOperation::Control(CheckedControl::Pattern {
                subject: subject.checked,
                arms: vec![CheckedPatternArm::new(pattern, branches.then_branch)]
                    .into_boxed_slice(),
                fallback,
                unmatched: else_syntax.is_none(),
            }),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    pub(super) fn check_match(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let subject_syntax = self.required_child(node, NodeKind::Expression)?;
        let subject = self.check_pattern_subject(subject_syntax)?;
        let variants = self.enum_variants(subject.checked.nominal())?;
        let mut covered = HashSet::new();
        let mut arms = Vec::new();
        let mut fallback = None;
        let mut branch_types = Vec::new();
        let mut inferred = expected;

        let arm_nodes = direct_children(self.tree(), node, NodeKind::MatchArm);
        if arm_nodes.is_empty() {
            return Err(self.rule(BodyRule::InvalidMatchCoverage, node)?);
        }
        for (position, arm_syntax) in arm_nodes.iter().copied().enumerate() {
            let block = self.required_child(arm_syntax, NodeKind::Block)?;
            if let Some(pattern_syntax) =
                direct_child(self.tree(), arm_syntax, NodeKind::EnumPattern)
            {
                if fallback.is_some() {
                    return Err(self.rule(BodyRule::InvalidMatchCoverage, arm_syntax)?);
                }
                let pattern = self.check_enum_pattern(pattern_syntax, &subject)?;
                if !covered.insert(pattern.variant()) {
                    return Err(self.rule(BodyRule::InvalidMatchCoverage, pattern_syntax)?);
                }
                let body = self.check_block(block, BlockExpectation::Value(inferred))?;
                let ty = self.node_type(body)?;
                inferred = self.branch_expectation(inferred, [ty]);
                branch_types.push(Some(ty));
                arms.push(CheckedPatternArm::new(pattern, body));
            } else {
                let wildcard = direct_identifier(self.tree(), arm_syntax)
                    .filter(|token| self.token_text(*token).ok() == Some("_"));
                if wildcard.is_none() || fallback.is_some() || position + 1 != arm_nodes.len() {
                    return Err(self.rule(BodyRule::InvalidMatchCoverage, arm_syntax)?);
                }
                let reachable = covered.len() != variants.len();
                let body = self.check_pattern_fallback(block, inferred, reachable)?;
                let ty = self.node_type(body)?;
                inferred = self.branch_expectation(inferred, [ty]);
                branch_types.push(Some(ty));
                fallback = Some(CheckedPatternFallback::new(body, reachable));
            }
        }

        if fallback.is_none() && covered.len() != variants.len() {
            return Err(self.rule(BodyRule::InvalidMatchCoverage, node)?);
        }
        let ty = self.branch_result_type(branch_types, false);
        let checked = self.add_node(
            node,
            ty,
            CheckedOperation::Control(CheckedControl::Pattern {
                subject: subject.checked,
                arms: arms.into_boxed_slice(),
                fallback,
                unmatched: false,
            }),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    fn check_pattern_subject(
        &mut self,
        root: NodeId,
    ) -> Result<PatternSubjectPlan, BodyCheckError> {
        let syntax = self.transparent_expression(root);
        let (value, place, explicit_move) = match self.kind(syntax)? {
            NodeKind::ReferenceExpression => {
                let place = self.named_place(syntax)?;
                let value = self.add_node(syntax, place.ty, CheckedOperation::Place(place.id))?;
                (value, true, false)
            }
            NodeKind::PostfixExpression
                if direct_child(self.tree(), syntax, NodeKind::CallSuffix).is_none() =>
            {
                let place = self.postfix_place(syntax, BorrowCapability::Readonly)?;
                let value = self.add_node(syntax, place.ty, CheckedOperation::Place(place.id))?;
                (value, true, false)
            }
            NodeKind::MoveExpression => {
                let plain_move = !self.tree().children(syntax).iter().any(|element| {
                    matches!(
                        element,
                        SyntaxElement::Token(token)
                            if matches!(
                                token.kind(),
                                TokenKind::Punctuation(Punctuation::Question | Punctuation::Bang)
                            )
                    )
                });
                (self.check_move(syntax)?, false, plain_move)
            }
            _ => (self.check_expression(syntax, None)?, false, false),
        };
        let ty = self.node_type(value)?;
        let (nominal_type, preparation) = match self.types.get(ty).cloned() {
            Some(TypeKind::Borrow {
                capability,
                referent,
            }) => (referent, PatternSubjectPreparation::Borrowed(capability)),
            Some(_) if explicit_move => (ty, PatternSubjectPreparation::ConsumedPlace),
            Some(_) if place => (ty, PatternSubjectPreparation::RetainedPlace),
            Some(_) => (ty, PatternSubjectPreparation::OwnedTemporary),
            None => return Err(BodyCheckInternalError::UnknownType(ty).into()),
        };
        let Some(TypeKind::Nominal {
            definition,
            arguments,
        }) = self.types.get(nominal_type).cloned()
        else {
            return Err(self.rule(BodyRule::InvalidPatternOperation, root)?);
        };
        let declaration = self
            .graph
            .declarations()
            .nominal_types()
            .get(definition)
            .ok_or(BodyCheckInternalError::UnknownType(nominal_type))?;
        if !matches!(declaration.shape(), NominalShape::Enum { .. })
            || declaration.generic_parameters().len() != arguments.len()
        {
            return Err(self.rule(BodyRule::InvalidPatternOperation, root)?);
        }
        Ok(PatternSubjectPlan {
            checked: CheckedPatternSubject::new(value, definition, preparation),
            arguments,
        })
    }

    fn check_enum_pattern(
        &mut self,
        node: NodeId,
        subject: &PatternSubjectPlan,
    ) -> Result<CheckedPattern, BodyCheckError> {
        let resolved = self.resolve_pattern_variant(node, subject)?;
        let slot_nodes = descendants(self.tree(), node, NodeKind::PayloadSlot);
        if resolved.parameters.len() != slot_nodes.len() {
            return Err(self.rule(BodyRule::InvalidPatternOperation, node)?);
        }
        self.check_pattern_slots(node, subject, resolved, slot_nodes)
    }

    fn resolve_pattern_variant(
        &mut self,
        node: NodeId,
        subject: &PatternSubjectPlan,
    ) -> Result<ResolvedPatternVariant, BodyCheckError> {
        let identifiers = identifier_tokens(self.tree(), node);
        let [owner_token, variant_token, ..] = identifiers.as_slice() else {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        };
        let owner_name = self.segment_symbol(*owner_token)?;
        let Some(owner) = self.graph.lookup_local(self.source.module(), owner_name) else {
            return Err(self.token_rule(BodyRule::InvalidPatternOperation, *owner_token)?);
        };
        let ExportedEntity::NominalType(nominal) = owner else {
            return Err(self.token_rule(BodyRule::InvalidPatternOperation, *owner_token)?);
        };
        if nominal != subject.checked.nominal() {
            return Err(self.token_rule(BodyRule::InvalidPatternOperation, *owner_token)?);
        }
        self.project_exported(*owner_token, owner)?;
        let variant_name = self.segment_symbol(*variant_token)?;
        let Some(variant) = self
            .construction_surfaces
            .variant(nominal, variant_name)
            .map_err(BodyCheckInternalError::from)?
        else {
            return Err(self.token_rule(BodyRule::InvalidPatternOperation, *variant_token)?);
        };
        self.project_type_entity(*variant_token, SemanticEntity::Variant(variant))?;
        let variant_declaration = self
            .graph
            .declarations()
            .variants()
            .get(variant)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let nominal_declaration = self
            .graph
            .declarations()
            .nominal_types()
            .get(nominal)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let mut substitution = TypeSubstitution::default();
        for (parameter, argument) in nominal_declaration
            .generic_parameters()
            .iter()
            .copied()
            .zip(subject.arguments.iter().copied())
        {
            substitution.bind_generic(parameter, argument);
        }
        Ok(ResolvedPatternVariant {
            nominal,
            variant,
            parameters: variant_declaration.payload().to_vec(),
            substitution,
        })
    }

    fn check_pattern_slots(
        &mut self,
        node: NodeId,
        subject: &PatternSubjectPlan,
        resolved: ResolvedPatternVariant,
        slot_nodes: Vec<NodeId>,
    ) -> Result<CheckedPattern, BodyCheckError> {
        let mut slots = Vec::with_capacity(resolved.parameters.len());
        let mut transfers_move_only = false;
        for (position, (parameter, slot)) in
            resolved.parameters.into_iter().zip(slot_nodes).enumerate()
        {
            let declaration = self
                .graph
                .declarations()
                .parameters()
                .get(parameter)
                .copied()
                .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
            if declaration.owner() != ParameterOwner::Variant(resolved.variant)
                || declaration.role()
                    != (ParameterRole::Ordinary {
                        position,
                        variadic: false,
                    })
            {
                return Err(BodyCheckInternalError::InvalidSyntax(node).into());
            }
            let token = direct_identifier(self.tree(), slot)
                .ok_or(BodyCheckInternalError::InvalidSyntax(slot))?;
            let binding = if self.token_text(token)? == "_" {
                None
            } else {
                let local = self
                    .local_declarations
                    .get(&SyntaxOrigin::Token(token))
                    .copied()
                    .ok_or(BodyCheckInternalError::MissingLocalDeclaration(slot))?;
                let payload = resolved
                    .substitution
                    .apply_type(self.types, declaration.ty())
                    .map_err(BodyCheckInternalError::CallSubstitution)?;
                let ty = self.pattern_binding_type(
                    slot,
                    payload,
                    subject.checked.preparation(),
                    &mut transfers_move_only,
                )?;
                self.builder.define_local(local, ty)?;
                Some(local)
            };
            slots.push(CheckedPatternSlot::new(parameter, binding));
        }
        let before_transfer_drop = (transfers_move_only
            && matches!(
                subject.checked.preparation(),
                PatternSubjectPreparation::OwnedTemporary
                    | PatternSubjectPreparation::ConsumedPlace
            ))
        .then(|| self.drops.get(resolved.nominal))
        .flatten();
        Ok(CheckedPattern::new(
            resolved.variant,
            slots,
            before_transfer_drop,
        ))
    }

    fn pattern_binding_type(
        &mut self,
        slot: NodeId,
        payload: TypeId,
        preparation: PatternSubjectPreparation,
        transfers_move_only: &mut bool,
    ) -> Result<TypeId, BodyCheckError> {
        match preparation {
            PatternSubjectPreparation::RetainedPlace => {
                if self
                    .copyabilities
                    .classify(self.graph, self.types, payload)
                    .map_err(BodyCheckInternalError::Copyability)?
                    != Copyability::Copy
                {
                    return Err(self.rule(BodyRule::InvalidPatternOperation, slot)?);
                }
                Ok(payload)
            }
            PatternSubjectPreparation::Borrowed(capability) => self
                .types
                .intern(TypeKind::Borrow {
                    capability,
                    referent: payload,
                })
                .map_err(|_| BodyCheckInternalError::UnknownType(payload).into()),
            PatternSubjectPreparation::OwnedTemporary
            | PatternSubjectPreparation::ConsumedPlace => {
                *transfers_move_only |= self
                    .copyabilities
                    .classify(self.graph, self.types, payload)
                    .map_err(BodyCheckInternalError::Copyability)?
                    == Copyability::MoveOnly;
                Ok(payload)
            }
        }
    }

    fn enum_variants(
        &self,
        nominal: NominalTypeId,
    ) -> Result<Box<[VariantId]>, BodyCheckInternalError> {
        let declaration = self
            .graph
            .declarations()
            .nominal_types()
            .get(nominal)
            .ok_or(BodyCheckInternalError::CleanupPlanning)?;
        let NominalShape::Enum { variants } = declaration.shape() else {
            return Err(BodyCheckInternalError::CleanupPlanning);
        };
        Ok(variants.clone())
    }

    fn transparent_expression(&self, root: NodeId) -> NodeId {
        let mut current = root;
        while self.kind(current).is_ok_and(is_transparent_expression) {
            let children = direct_nodes(self.tree(), current);
            let [child] = children.as_slice() else {
                break;
            };
            current = *child;
        }
        current
    }

    fn check_pattern_fallback(
        &mut self,
        block: NodeId,
        expected: Option<TypeId>,
        reachable: bool,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let previous = self.flow_reachable;
        self.flow_reachable &= reachable;
        let result = self.check_block(block, BlockExpectation::Value(expected));
        self.flow_reachable = previous;
        result
    }

    fn branch_expectation(
        &self,
        expected: Option<TypeId>,
        types: impl IntoIterator<Item = TypeId>,
    ) -> Option<TypeId> {
        let never = self.types.builtin(BuiltinType::Never);
        expected.or_else(|| types.into_iter().find(|ty| *ty != never))
    }

    fn branch_result_type(
        &self,
        branches: impl IntoIterator<Item = Option<TypeId>>,
        has_implicit_void: bool,
    ) -> TypeId {
        if has_implicit_void {
            return self.types.builtin(BuiltinType::Void);
        }
        let never = self.types.builtin(BuiltinType::Never);
        branches
            .into_iter()
            .flatten()
            .find(|ty| *ty != never)
            .unwrap_or(never)
    }
}
