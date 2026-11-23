//! Construct the case from exact declaration-ordered owning arguments.
use super::{Checker, ExpressionKind, Failure, FieldValue, Mode, Rule, value_type};
use crate::value_access::qualified::EnumVariant;
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{AccessExpressionId, FrozenData, FrozenNode};
use nocter_syntax::{NodeId, SyntaxOrigin, SyntaxToken};

impl Checker<'_, '_> {
    pub(in crate::value_access) fn enum_value(
        &mut self,
        node: NodeId,
        member: SyntaxToken,
        variant: EnumVariant,
        arguments: Option<&[NodeId]>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let EnumVariant { data, variant, tag } = variant;
        let components = self
            .program()
            .variant_components(data, variant)
            .ok_or(Failure::InconsistentSyntax)?
            .collect::<Vec<_>>();
        if arguments.is_some() && components.is_empty() {
            return self.reject_message(
                Rule::InvalidCall,
                member,
                "use a payloadless enum variant without parentheses".into(),
                Vec::new(),
            );
        }
        let arguments = arguments.unwrap_or_default();
        if arguments.len() != components.len() {
            return self.reject_message(
                Rule::InvalidConstruction,
                member,
                format!(
                    "this enum variant requires {} positional arguments",
                    components.len()
                ),
                Vec::new(),
            );
        }
        let Some(PayloadSchema::Enumeration(schema)) = self.program().data_schema(data) else {
            return Err(Failure::InconsistentSyntax);
        };
        if !schema.has_payload() {
            let value = FrozenData::new(vec![FrozenNode::Enum {
                tag,
                fields: Box::new([]),
            }])
            .map_err(|_| Failure::InconsistentSyntax)?;
            return self.constant_value(node, data, &value);
        }
        super::super::checker::payload_requirements::data(
            self.program(),
            self.syntax(),
            data,
            SyntaxOrigin::Token(member),
        )?;
        if self.program().instantiate_closed_data(data).is_err() {
            return self.reject(Rule::UnsupportedData, member);
        }
        let mut fields = Vec::with_capacity(components.len());
        let mut continues = true;
        for (&source, component) in arguments.iter().zip(components) {
            let value =
                self.construction_value(source, Some(component.data), reachable && continues)?;
            continues &= self.ty(value).may_complete();
            fields.push(FieldValue {
                field: component.field,
                value,
            });
        }
        self.emit(
            node,
            value_type(data, Mode::Own).with_continuation(continues),
            ExpressionKind::Aggregate {
                enum_case: Some(tag),
                fields: fields.into_boxed_slice(),
            },
        )
    }
}
