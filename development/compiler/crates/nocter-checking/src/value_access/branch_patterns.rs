//! Enum patterns select a variant in the type namespace before inspecting the source.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Boundary, Projection},
    statements::{BranchBindings, Leaf},
};
use nocter_declaration_lowering::access_declarations::input_operation;
use nocter_model::{
    AccessExpressionId, BuiltinType, FieldPath, TypeId, ValueAccess, ValueInputOperation,
};
use nocter_syntax::{NodeId, NodeKind, TokenKind, direct_token_iter};

mod matching;

#[derive(Clone, Copy)]
pub(super) enum IfTest {
    Boolean(AccessExpressionId),
    Enum {
        source: AccessExpressionId,
        operation: ValueInputOperation,
        tag: u64,
    },
}
impl IfTest {
    pub(super) fn fallback_bindings(self) -> Option<BranchBindings> {
        match self {
            Self::Boolean(_) => None,
            Self::Enum {
                source, operation, ..
            } => BranchBindings::for_input(source, operation, Vec::new()),
        }
    }
}

struct TagPattern {
    data: TypeId,
    tag: u64,
    leaves: Vec<Leaf>,
}

impl Checker<'_, '_> {
    pub(super) fn if_test(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<(IfTest, Option<BranchBindings>), Failure> {
        let Ok(pattern) = self.direct(node, NodeKind::EnumPattern) else {
            let source_node = self.direct(node, NodeKind::Expression)?;
            let condition = self.expression(source_node, None, reachable)?;
            self.require_data(condition, self.data(BuiltinType::Bool), source_node)?;
            return Ok((IfTest::Boolean(condition), None));
        };
        let pattern = self.tag_pattern(pattern)?;
        let (source, operation) = self.pattern_input(
            node,
            Some(pattern.data),
            !pattern.leaves.is_empty(),
            reachable,
        )?;
        let test = IfTest::Enum {
            source,
            operation,
            tag: pattern.tag,
        };
        Ok((
            test,
            BranchBindings::for_input(source, operation, pattern.leaves),
        ))
    }

    fn tag_pattern(&self, pattern: NodeId) -> Result<TagPattern, Failure> {
        let path = direct_token_iter(self.syntax().tree(), pattern)
            .filter(|token| token.kind() == TokenKind::Identifier)
            .collect::<Vec<_>>();
        let (&member, owner) = path.split_last().ok_or(Failure::InconsistentSyntax)?;
        let data = self.annotation_data_path(owner, false)?;
        let variant = self
            .program()
            .behavior_owner(data)
            .map(|owner| self.enum_variant(owner, member))
            .transpose()?
            .flatten();
        let Some(super::qualified::EnumVariant { data, variant, tag }) = variant else {
            return self.reject_message(
                Rule::UnknownName,
                member,
                format!(
                    "{} has no variant `{}`",
                    self.data_label(data),
                    self.text(member)?
                ),
                Vec::new(),
            );
        };
        let components = self
            .program()
            .variant_components(data, variant)
            .ok_or(Failure::InconsistentSyntax)?;
        let arity = components.len();
        let payload = self.direct(pattern, NodeKind::EnumPatternPayload).ok();
        if arity == 0
            && let Some(payload) = payload
        {
            return self.reject_message(
                Rule::TypeMismatch,
                payload,
                "a payloadless enum pattern has no bindings".into(),
                Vec::new(),
            );
        }
        let written = payload.map_or_else(Vec::new, |node| self.children(node));
        if written.len() != arity {
            return self.reject_message(
                Rule::TypeMismatch,
                payload.unwrap_or(pattern),
                format!("this enum pattern requires {arity} payload positions"),
                Vec::new(),
            );
        }
        let roots = written
            .into_iter()
            .zip(components)
            .map(|(node, component)| (node, component.data, FieldPath::new([component.field])));
        Ok(TagPattern {
            data,
            tag,
            leaves: self.binding_leaves(roots)?,
        })
    }

    fn pattern_input(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
        binds_payload: bool,
        reachable: bool,
    ) -> Result<(AccessExpressionId, ValueInputOperation), Failure> {
        let source_node = self.direct(node, NodeKind::Expression)?;
        let operation =
            input_operation(self.syntax(), node).map_err(|_| Failure::InconsistentSyntax)?;
        let mut source = self.expression_with_type_hint(
            source_node,
            expected,
            Some(Projection::input(operation.access_mode())),
            reachable,
        )?;
        if let Some(data) = expected {
            self.require_data(source, data, source_node)?;
        }
        match operation {
            ValueInputOperation::Take => {
                if let Some(data) = self.ty(source).access().map(ValueAccess::data) {
                    source = self.boundary(source_node, source, Boundary::Value(data))?;
                }
            }
            ValueInputOperation::Read | ValueInputOperation::Edit => {
                if let Some(place) = self.expression_referent(source) {
                    if operation == ValueInputOperation::Edit {
                        self.require_writable_referent(place, source_node, Rule::InvalidUpdate)?;
                    }
                } else if binds_payload
                    && let Some(data) = self.ty(source).access().map(ValueAccess::data)
                {
                    source = self.boundary(
                        source_node,
                        source,
                        Boundary::Input(ValueAccess::new(data, operation.access_mode())),
                    )?;
                }
            }
        }
        Ok((source, operation))
    }
}
