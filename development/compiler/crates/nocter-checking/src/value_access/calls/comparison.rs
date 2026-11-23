//! Fixed nominal comparisons retain written preparation order and the selected formal inputs.
use super::{CallArguments, CallKind, Callee, Checker, Failure, Rule, value_type};
use crate::value_access::ir::{Binary, ExpressionKind, Unary};
use nocter_declaration_lowering::access_declarations::ComparisonFamily;
use nocter_declarations::value_access::BehaviorOwner;
use nocter_model::{AccessExpressionId, NominalTypeId, ValueAccessMode as Mode};
use nocter_syntax::NodeId;

impl Checker<'_, '_> {
    pub(in crate::value_access) fn nominal_comparison(
        &mut self,
        node: NodeId,
        operation: Binary,
        record: NominalTypeId,
        operands: [(NodeId, AccessExpressionId); 2],
    ) -> Result<AccessExpressionId, Failure> {
        let token = self.binary_token(node)?;
        let (family, invert) = match operation {
            Binary::Equal => (ComparisonFamily::Equality, false),
            Binary::NotEqual => (ComparisonFamily::Equality, true),
            Binary::Less | Binary::Greater => (ComparisonFamily::Strict, false),
            _ => {
                return self.reject_message(
                    Rule::TypeMismatch,
                    token,
                    "this nominal comparison form is not yet supported".into(),
                    Vec::new(),
                );
            }
        };
        let Some(member) = self
            .context
            .comparison(BehaviorOwner::Record(record), family)
        else {
            return self.reject_message(
                Rule::TypeMismatch,
                token,
                "the operand type does not declare the required comparison".into(),
                Vec::new(),
            );
        };
        self.require_type_member(member, token, true)?;
        let selected = Callee::direct(member.code, token, None);
        let contract = self
            .program()
            .function_contract(selected.witness)
            .ok_or(Failure::InconsistentSyntax)?;
        let [left, right] = contract.inputs.as_ref() else {
            return Err(Failure::InconsistentSyntax);
        };
        let continues = operands
            .iter()
            .all(|&(_, operand)| self.ty(operand).may_complete());
        // Check the exact declared read pair. Neither an implicit conversion nor an outcome
        // injection can turn a different operand type into the comparison's selected Self.
        let [(left_syntax, left_value), (right_syntax, right_value)] = operands;
        let plans = [
            self.input_plan(left_syntax, left_value, *left, None)?,
            self.input_plan(right_syntax, right_value, *right, None)?,
        ];
        let arguments = if operation == Binary::Greater {
            CallArguments::reverse_pair(plans)
        } else {
            CallArguments::in_order(plans)
        };
        let result = self.emit_selected_call(
            node,
            &selected,
            &contract,
            CallKind::Direct,
            arguments,
            continues,
        )?;
        if invert {
            self.emit(
                node,
                value_type(contract.result, Mode::Own).with_continuation(continues),
                ExpressionKind::Unary {
                    operation: Unary::Not,
                    operand: result,
                },
            )
        } else {
            Ok(result)
        }
    }
}
