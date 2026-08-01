use crate::abi::AbiType;
use crate::ast::{ArrayLiteralExpr, Expr};
use crate::diagnostics::Diagnostic;
use crate::ir::lower::context::{
    AggregateDrop, ArrayElementDropState, DropObligation, PayloadFieldDropState,
    StructFieldDropState,
};
use crate::ir::{BoolValue, Instruction, UsizeLocation, UsizeValue};

use super::{
    DropStateAllocator, PayloadInitializationProgress, StructInitializationProgress,
    payload_enum_constructor_member_and_arguments,
};

pub(in crate::ir::lower) fn array_literal_requires_runtime_progress(
    literal: &ArrayLiteralExpr,
) -> bool {
    literal.elements.iter().any(expression_propagates_failure)
}

fn expression_propagates_failure(expression: &Expr) -> bool {
    match expression {
        Expr::Propagate(_) => true,
        Expr::Group(group) => expression_propagates_failure(&group.expression),
        Expr::ArrayLiteral(literal) => literal.elements.iter().any(expression_propagates_failure),
        Expr::StructLiteral(literal) => literal
            .fields
            .iter()
            .any(|field| expression_propagates_failure(&field.value)),
        Expr::Call(call) => call.arguments.iter().any(expression_propagates_failure),
        _ => false,
    }
}

/// Runtime ownership state for left-to-right fixed-array initialization.
///
/// `initialized` counts fully published elements. `elements` describes the
/// recursive state that may be live while each next element is being built.
/// Cleanup selects the state whose index equals `initialized`, drops it, then
/// drops the completed prefix in reverse order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::ir::lower) struct ArrayInitializationProgress {
    initialized: UsizeLocation,
    elements: Vec<ArrayElementDropState>,
}

impl ArrayInitializationProgress {
    #[cfg(test)]
    pub(in crate::ir::lower) fn new(initialized: UsizeLocation) -> Self {
        Self {
            initialized,
            elements: Vec::new(),
        }
    }

    pub(in crate::ir::lower) fn with_allocator(
        literal: &ArrayLiteralExpr,
        element_type: &AbiType,
        drop_kind: &AggregateDrop,
        initialized: UsizeLocation,
        allocator: &mut impl DropStateAllocator,
    ) -> Result<Self, Vec<Diagnostic>> {
        let AggregateDrop::Array(array_drop) = drop_kind else {
            return Err(invalid_array_initialization_state_diagnostic());
        };
        if usize::try_from(array_drop.length).ok() != Some(literal.elements.len()) {
            return Err(invalid_array_initialization_state_diagnostic());
        }

        let mut elements = Vec::new();
        for (index, expression) in literal.elements.iter().enumerate() {
            let partial = partial_drop_obligation_for_initializer(
                expression,
                element_type,
                array_drop.element_drop_kind.as_ref(),
                allocator,
            )?;
            if partial.is_active() {
                elements.push(ArrayElementDropState {
                    index: u64::try_from(index)
                        .map_err(|_error| invalid_array_initialization_state_diagnostic())?,
                    partial: Box::new(partial),
                });
            }
        }
        Ok(Self {
            initialized,
            elements,
        })
    }

    pub(in crate::ir::lower) fn from_drop_state(
        initialized: UsizeLocation,
        elements: Vec<ArrayElementDropState>,
    ) -> Self {
        Self {
            initialized,
            elements,
        }
    }

    pub(in crate::ir::lower) fn location(&self) -> UsizeLocation {
        self.initialized
    }

    pub(in crate::ir::lower) fn element_states(&self) -> Vec<ArrayElementDropState> {
        self.elements.clone()
    }

    pub(in crate::ir::lower) fn element_obligation(&self, index: u64) -> Option<&DropObligation> {
        self.elements
            .iter()
            .find(|state| state.index == index)
            .map(|state| state.partial.as_ref())
    }

    pub(in crate::ir::lower) fn drop_obligation(&self) -> DropObligation {
        DropObligation::ArrayPrefix {
            initialized: self.initialized,
            elements: self.element_states(),
        }
    }

    pub(in crate::ir::lower) fn initialize(&self) -> Vec<Instruction> {
        initialize_drop_obligation(&self.drop_obligation())
    }

    pub(in crate::ir::lower) fn complete_element(&self, initialized_count: u64) -> Instruction {
        Instruction::SetUsize {
            destination: self.initialized,
            value: UsizeValue::Const(initialized_count),
        }
    }
}

pub(in crate::ir::lower) fn partial_drop_obligation_for_initializer(
    expression: &Expr,
    value_type: &AbiType,
    drop_kind: &AggregateDrop,
    allocator: &mut impl DropStateAllocator,
) -> Result<DropObligation, Vec<Diagnostic>> {
    match (unwrap_groups(expression), drop_kind, value_type) {
        (Expr::ArrayLiteral(literal), AggregateDrop::Array(_), AbiType::Array { element, .. }) => {
            let initialized = allocator.next_drop_usize()?;
            ArrayInitializationProgress::with_allocator(
                literal,
                element,
                drop_kind,
                initialized,
                allocator,
            )
            .map(|progress| progress.drop_obligation())
        }
        (
            Expr::StructLiteral(literal),
            AggregateDrop::Direct(_) | AggregateDrop::Struct(_),
            AbiType::Struct(fields),
        ) => StructInitializationProgress::with_allocator(fields, literal, drop_kind, allocator)
            .map(|progress| DropObligation::StructFields {
                fields: progress.drop_states(),
            }),
        (_, AggregateDrop::PayloadEnum(_), AbiType::Enum(enum_))
            if payload_enum_constructor_member_and_arguments(expression).is_some() =>
        {
            PayloadInitializationProgress::with_allocator(expression, enum_, drop_kind, allocator)
                .map(|progress| progress.drop_obligation())
        }
        _ => Ok(DropObligation::Inactive),
    }
}

pub(in crate::ir::lower) fn initialize_drop_obligation(
    obligation: &DropObligation,
) -> Vec<Instruction> {
    let mut instructions = Vec::new();
    match obligation {
        DropObligation::ArrayPrefix {
            initialized,
            elements,
        } => {
            instructions.push(Instruction::SetUsize {
                destination: *initialized,
                value: UsizeValue::Const(0),
            });
            for element in elements {
                instructions.extend(initialize_drop_obligation(element.partial.as_ref()));
            }
        }
        DropObligation::StructFields { fields } => {
            initialize_struct_fields(fields, &mut instructions);
        }
        DropObligation::PayloadFields { fields, .. } => {
            initialize_payload_fields(fields, &mut instructions);
        }
        DropObligation::Inactive | DropObligation::Complete => {}
    }
    instructions
}

fn initialize_struct_fields(fields: &[StructFieldDropState], instructions: &mut Vec<Instruction>) {
    for field in fields {
        instructions.push(Instruction::SetBool {
            destination: field.initialized,
            value: BoolValue::Const(false),
        });
        instructions.extend(initialize_drop_obligation(field.partial.as_ref()));
    }
}

fn initialize_payload_fields(
    fields: &[PayloadFieldDropState],
    instructions: &mut Vec<Instruction>,
) {
    for field in fields {
        instructions.push(Instruction::SetBool {
            destination: field.initialized,
            value: BoolValue::Const(false),
        });
        instructions.extend(initialize_drop_obligation(field.partial.as_ref()));
    }
}

fn unwrap_groups(mut expression: &Expr) -> &Expr {
    while let Expr::Group(group) = expression {
        expression = &group.expression;
    }
    expression
}

fn invalid_array_initialization_state_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8008",
        "IR cannot establish recursive array initialization state",
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_starts_empty_and_advances_after_a_complete_element() {
        let progress = ArrayInitializationProgress::new(UsizeLocation::Local(4));

        assert_eq!(
            progress.initialize(),
            vec![Instruction::SetUsize {
                destination: UsizeLocation::Local(4),
                value: UsizeValue::Const(0),
            }]
        );
        assert_eq!(
            progress.complete_element(2),
            Instruction::SetUsize {
                destination: UsizeLocation::Local(4),
                value: UsizeValue::Const(2),
            }
        );
    }
}
