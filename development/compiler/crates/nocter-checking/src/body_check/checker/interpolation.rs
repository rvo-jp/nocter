use nocter_model::{
    BorrowCapability, BuiltinType, CallableCapability, CallableId, InterfaceId, NominalTypeId,
    TypeId, TypeKind,
};
use nocter_syntax::{DecodedStringPart, NodeId};
use nocter_toolchain_contract::StandardDeclarationRole;

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::{
    AllocationSelection, CheckedInterpolation, CheckedOperation, CheckedReadonlyOperand,
    ConstantValue, GenericArguments, InterpolationPart, StaticDispatch, StaticSelection,
};

struct InterpolationSemantics {
    string: NominalTypeId,
    constructor: CallableId,
    text_appender: CallableId,
    format: InterfaceId,
    format_method: CallableId,
}

impl BodyChecker<'_, '_> {
    pub(super) fn check_string_expression(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let source = self
            .input
            .sources()
            .get(self.tree().source())
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let parts = nocter_syntax::decode_string_expression(source, self.tree(), node)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        if parts
            .iter()
            .all(|part| matches!(part, DecodedStringPart::Text(_)))
        {
            return self.check_static_string(node, parts, expected);
        }
        self.check_interpolation(node, parts, expected)
    }

    fn check_static_string(
        &mut self,
        node: NodeId,
        parts: Box<[DecodedStringPart]>,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let mut text = String::new();
        for part in parts {
            let DecodedStringPart::Text(part) = part else {
                return Err(BodyCheckInternalError::InvalidSyntax(node).into());
            };
            text.push_str(&part);
        }
        let referent = self.types.builtin(BuiltinType::Str);
        let ty = self
            .types
            .intern(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            })
            .map_err(|_| BodyCheckInternalError::UnknownType(referent))?;
        let checked = self.add_node(
            node,
            ty,
            CheckedOperation::Constant(ConstantValue::Text(text.into_boxed_str())),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    fn check_interpolation(
        &mut self,
        node: NodeId,
        parts: Box<[DecodedStringPart]>,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let semantics = self.interpolation_semantics()?;
        let string_type = self
            .types
            .intern(TypeKind::Nominal {
                definition: semantics.string,
                arguments: Box::default(),
            })
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        let never = self.types.builtin(BuiltinType::Never);
        let mut diverges = false;
        let mut checked_parts = Vec::with_capacity(parts.len());
        for part in parts {
            match part {
                DecodedStringPart::Text(text) => {
                    checked_parts.push(InterpolationPart::Text(text));
                }
                DecodedStringPart::Expression(expression) => {
                    let operand = self.check_readonly_operand(expression, None)?;
                    if operand.ty == never {
                        diverges = true;
                        checked_parts.push(InterpolationPart::Diverging(operand.value));
                        continue;
                    }
                    let candidates = {
                        let mut selector = self.instance_selector();
                        selector
                            .select_exact_interface_method(
                                operand.owner,
                                semantics.format,
                                semantics.format_method,
                            )
                            .map_err(BodyCheckInternalError::from)?
                    };
                    let [formatter] = candidates.as_slice() else {
                        return Err(self.rule(BodyRule::InvalidInterpolation, expression)?);
                    };
                    if formatter.receiver_capability() != CallableCapability::Readonly {
                        return Err(
                            BodyCheckInternalError::MissingInterpolationSemanticRoles.into()
                        );
                    }
                    checked_parts.push(InterpolationPart::Formatted {
                        operand: CheckedReadonlyOperand::new(
                            operand.value,
                            operand.preparation,
                            None,
                        ),
                        formatter: StaticSelection::new(
                            formatter.dispatch(),
                            formatter.generic_arguments().clone(),
                        ),
                    });
                }
            }
        }
        let result = if diverges { never } else { string_type };
        let checked = self.add_node(
            node,
            result,
            CheckedOperation::Interpolation(CheckedInterpolation::new(
                StaticSelection::new(
                    StaticDispatch::Direct(semantics.constructor),
                    GenericArguments::default(),
                ),
                StaticSelection::new(
                    StaticDispatch::Direct(semantics.text_appender),
                    GenericArguments::default(),
                ),
                checked_parts,
                string_type,
                AllocationSelection::CurrentRegion,
            )),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    fn interpolation_semantics(&self) -> Result<InterpolationSemantics, BodyCheckError> {
        let semantics = InterpolationSemantics {
            string: self
                .standard_semantics
                .nominal(StandardDeclarationRole::OwnedString)
                .ok_or(BodyCheckInternalError::MissingInterpolationSemanticRoles)?,
            constructor: self
                .standard_semantics
                .callable(StandardDeclarationRole::InterpolationConstructor)
                .ok_or(BodyCheckInternalError::MissingInterpolationSemanticRoles)?,
            text_appender: self
                .standard_semantics
                .callable(StandardDeclarationRole::InterpolationTextAppender)
                .ok_or(BodyCheckInternalError::MissingInterpolationSemanticRoles)?,
            format: self
                .standard_semantics
                .interface(StandardDeclarationRole::FormatInterface)
                .ok_or(BodyCheckInternalError::MissingInterpolationSemanticRoles)?,
            format_method: self
                .standard_semantics
                .callable(StandardDeclarationRole::FormatMethod)
                .ok_or(BodyCheckInternalError::MissingInterpolationSemanticRoles)?,
        };
        Ok(semantics)
    }
}
