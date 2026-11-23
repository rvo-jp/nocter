//! Check enum coverage and construct one multiway branch over the selected carrier.
use super::{Checker, Failure, Rule, TagPattern};
use crate::value_access::{
    annotations::Annotation,
    ir::{Boundary, ExpressionKind, ValueType},
    statements::BranchBindings,
};
use nocter_model::{AccessBlockId, AccessExpressionId, TypeId, ValueAccess, ValueAccessMode};
use nocter_syntax::{NodeId, NodeKind};
use std::collections::BTreeSet;

struct Arms {
    data: Option<TypeId>,
    cases: Vec<(TagPattern, NodeId)>,
    fallback: Option<NodeId>,
}

impl Checker<'_, '_> {
    pub(in crate::value_access) fn enum_match(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
        hint: Option<&Annotation>,
        data_hint: Option<TypeId>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let arms = self.enum_arms(node)?;
        let binds_payload = arms
            .cases
            .iter()
            .any(|(pattern, _)| !pattern.leaves.is_empty());
        let (source, operation) = self.pattern_input(node, arms.data, binds_payload, reachable)?;
        let source_ty = self.ty(source);
        let data = source_ty.access().map(ValueAccess::data).or(arms.data);
        self.require_enum_coverage(node, data, &arms)?;
        let branch_reachable = reachable && source_ty.may_complete();
        let owning_boundary =
            expected.filter(|boundary| boundary.ty().mode() == ValueAccessMode::Own);
        let branch_boundary = owning_boundary.filter(|_| source_ty.may_complete());
        let construction = if expected.is_none() { data_hint } else { None };
        let check = |this: &mut Self, block, bindings| {
            this.block_with_case(
                block,
                branch_boundary,
                hint,
                construction,
                branch_reachable,
                bindings,
            )
        };
        let mut cases: Vec<_> = arms
            .cases
            .into_iter()
            .map(|(pattern, block)| {
                let tag = pattern.tag;
                let bindings = BranchBindings::for_input(source, operation, pattern.leaves);
                Ok((tag, check(self, block, bindings)?))
            })
            .collect::<Result<_, Failure>>()?;
        let fallback = arms
            .fallback
            .map(|block| {
                check(
                    self,
                    block,
                    BranchBindings::for_input(source, operation, Vec::new()),
                )
            })
            .transpose()?;
        let blocks = || cases.iter().map(|&(_, block)| block).chain(fallback);
        let ty = self.enum_match_type(node, source_ty, blocks())?;
        if construction.is_some()
            && let Some(access) = ty.normal()
        {
            self.supply_conditional_results(node, blocks(), access)?;
        }
        cases.sort_unstable_by_key(|&(tag, _)| tag);
        let value = self.emit(
            node,
            ty,
            ExpressionKind::EnumMatch {
                source,
                operation,
                cases: cases.into_boxed_slice(),
                fallback,
            },
        )?;
        if let Some(boundary) = expected.filter(|_| owning_boundary.is_none()) {
            self.boundary(node, value, boundary)
        } else {
            Ok(value)
        }
    }

    fn enum_arms(&self, node: NodeId) -> Result<Arms, Failure> {
        let mut arms = Arms {
            data: None,
            cases: Vec::new(),
            fallback: None,
        };
        let mut tags = BTreeSet::new();
        for arm in nocter_syntax::direct_node_iter(self.syntax().tree(), node, NodeKind::MatchArm) {
            if arms.fallback.is_some() {
                return self.reject_message(
                    Rule::TypeMismatch,
                    arm,
                    "the discard arm must be last".into(),
                    Vec::new(),
                );
            }
            let block = self.direct(arm, NodeKind::Block)?;
            let Ok(pattern) = self.direct(arm, NodeKind::EnumPattern) else {
                arms.fallback = Some(block);
                continue;
            };
            let selected = self.tag_pattern(pattern)?;
            if arms.data.is_some_and(|owner| owner != selected.data) {
                return self.reject_message(
                    Rule::TypeMismatch,
                    pattern,
                    "every match arm must name a variant of the same enum".into(),
                    Vec::new(),
                );
            }
            if !tags.insert(selected.tag) {
                return self.reject_message(
                    Rule::TypeMismatch,
                    pattern,
                    "this variant already has a match arm".into(),
                    Vec::new(),
                );
            }
            arms.data = Some(selected.data);
            arms.cases.push((selected, block));
        }
        Ok(arms)
    }

    fn require_enum_coverage(
        &self,
        node: NodeId,
        data: Option<TypeId>,
        arms: &Arms,
    ) -> Result<(), Failure> {
        let complete = if let Some(data) = data {
            let program = self.program();
            let Some((owner, schema)) = program
                .nominal_identity(data)
                .and_then(|owner| program.enumeration(owner).map(|schema| (owner, schema)))
            else {
                return self.reject_message(
                    Rule::TypeMismatch,
                    self.direct(node, NodeKind::Expression)?,
                    "match input must be a nominal enum value".into(),
                    Vec::new(),
                );
            };
            self.context.enum_schema_visible(owner) == Some(true)
                && arms.cases.len() == schema.variants().len()
        } else {
            false
        };
        match (complete, arms.fallback) {
            (true, Some(block)) => self.reject_message(
                Rule::TypeMismatch,
                block,
                "all variants are already covered before the discard arm".into(),
                Vec::new(),
            ),
            (false, None) => self.reject_message(
                Rule::TypeMismatch,
                node,
                "match must cover the complete visible enum or end with a discard arm".into(),
                Vec::new(),
            ),
            _ => Ok(()),
        }
    }

    fn enum_match_type(
        &mut self,
        node: NodeId,
        source: ValueType,
        blocks: impl Iterator<Item = AccessBlockId>,
    ) -> Result<ValueType, Failure> {
        let mut checked = None;
        let mut producing = None;
        for block in blocks {
            let ty = self.block_ty(block);
            checked = self.merge_branch_access(node, checked, ty.access())?;
            producing = producing.or(ty.normal());
        }
        Ok(producing
            .or(checked)
            .map_or(ValueType::NEVER, ValueType::known)
            .with_continuation(source.may_complete() && producing.is_some()))
    }
}
