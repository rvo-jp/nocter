//! Fixed nominal comparisons retain written preparation order and the selected formal inputs.
use super::{
    Argument, ArgumentAction, CallArguments, CallKind, Callee, CalleeSource, Checker, Failure,
    Rule, value_type,
};
use crate::value_access::ir::{Binary, ExpressionKind, Unary};
use nocter_declaration_lowering::access_declarations::ComparisonFamily;
use nocter_declarations::value_access::{BehaviorOwner, FunctionConstraint};
use nocter_model::{
    AccessExpressionId, NominalTypeId, ValueAccessMode as Mode, ValueInputOperation,
};
use nocter_syntax::{NodeId, SyntaxToken};

impl Checker<'_, '_> {
    pub(in crate::value_access) fn nominal_comparison(
        &mut self,
        node: NodeId,
        operation: Binary,
        record: NominalTypeId,
        operands: [(NodeId, AccessExpressionId); 2],
    ) -> Result<AccessExpressionId, Failure> {
        let token = self.binary_token(node)?;
        let family = match operation {
            Binary::Equal | Binary::NotEqual => ComparisonFamily::Equality,
            Binary::Less | Binary::Greater | Binary::LessEqual | Binary::GreaterEqual => {
                ComparisonFamily::Strict
            }
            _ => return Err(Failure::InconsistentSyntax),
        };
        let (selected, contract) = self.comparison_entry(record, family, token)?;
        let inclusive = matches!(operation, Binary::LessEqual | Binary::GreaterEqual);
        let equality = inclusive
            .then(|| self.comparison_entry(record, ComparisonFamily::Equality, token))
            .transpose()?;
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
        let mut arguments = if matches!(operation, Binary::Greater | Binary::GreaterEqual) {
            CallArguments::reverse_pair(plans)
        } else {
            CallArguments::in_order(plans)
        };
        let result_type = value_type(contract.result, Mode::Own).with_continuation(continues);
        if let Some((equality, equal_contract)) = equality {
            if contract != equal_contract || !arguments.retain_read_pair() {
                return Err(Failure::InconsistentSyntax);
            }
            return self.inclusive_comparison(
                node,
                result_type,
                arguments,
                (&selected, &contract),
                (equality, &equal_contract),
            );
        }
        let result = self.emit_selected_call(
            node,
            &selected,
            &contract,
            CallKind::Direct,
            arguments,
            continues,
        )?;
        if operation == Binary::NotEqual {
            self.emit(
                node,
                result_type,
                ExpressionKind::Unary {
                    operation: Unary::Not,
                    operand: result,
                },
            )
        } else {
            Ok(result)
        }
    }

    fn comparison_entry(
        &self,
        record: NominalTypeId,
        family: ComparisonFamily,
        token: SyntaxToken,
    ) -> Result<(Callee, FunctionConstraint), Failure> {
        let Some(member) = self
            .context
            .comparison(BehaviorOwner::Nominal(record), family)
        else {
            let entry = match family {
                ComparisonFamily::Equality => "equality",
                ComparisonFamily::Strict => "strict comparison",
            };
            return self.reject_message(
                Rule::TypeMismatch,
                token,
                format!("the operand type does not declare the required {entry} entry"),
                Vec::new(),
            );
        };
        self.require_type_member(member, token, true)?;
        let selected = Callee::direct(member.code, token, None);
        let contract = self
            .program()
            .function_contract(selected.witness)
            .ok_or(Failure::InconsistentSyntax)?;
        Ok((selected, contract))
    }

    fn inclusive_comparison(
        &mut self,
        node: NodeId,
        result_type: crate::value_access::ValueType,
        arguments: CallArguments,
        (strict, strict_contract): (&Callee, &FunctionConstraint),
        (mut equality, equal_contract): (Callee, &FunctionConstraint),
    ) -> Result<AccessExpressionId, Failure> {
        let continues = result_type.may_complete();
        // Equality always uses the written left as receiver, even when strict used it as other.
        let written = {
            let mut inputs = arguments.preparation_order();
            [
                inputs.next().ok_or(Failure::InconsistentSyntax)?,
                inputs.next().ok_or(Failure::InconsistentSyntax)?,
            ]
            .map(|(index, input)| (index, input.value))
        };
        let first = self.emit_selected_call(
            node,
            strict,
            strict_contract,
            CallKind::Direct,
            arguments,
            continues,
        )?;
        let retained = written.map(|(index, value)| Argument {
            operation: ValueInputOperation::Read,
            binding: None,
            value,
            action: if continues {
                ArgumentAction::ReuseRead { call: first, index }
            } else {
                ArgumentAction::Interrupted
            },
        });
        equality.source = CalleeSource::Declared {
            token: None,
            receiver: None,
        };
        let second = self.emit_selected_call(
            node,
            &equality,
            equal_contract,
            CallKind::Direct,
            CallArguments::in_order(retained),
            continues,
        )?;
        self.emit(
            node,
            result_type,
            ExpressionKind::Binary {
                operation: Binary::Or,
                left: first,
                right: second,
            },
        )
    }
}
