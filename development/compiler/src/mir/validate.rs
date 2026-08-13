//! Structural MIR verification. Validation errors are compiler invariant
//! failures, not alternate source-language diagnostics.

use super::ids::{BasicBlockId, LocalId};
use super::model::{Body, CallContinuation, Operand, ScalarType, Statement, Terminator};
use crate::semantic::TyId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ValidationError {
    MissingReturnLocal(LocalId),
    MissingEntryBlock(BasicBlockId),
    MissingAssignmentLocal {
        block: BasicBlockId,
        statement: usize,
        local: LocalId,
    },
    AssignmentTypeMismatch {
        block: BasicBlockId,
        statement: usize,
        destination: TyId,
        value: TyId,
    },
    MissingOperandLocal {
        block: BasicBlockId,
        location: OperandLocation,
        local: LocalId,
    },
    OperandTypeMismatch {
        block: BasicBlockId,
        location: OperandLocation,
        expected: TyId,
        actual: TyId,
    },
    OperandScalarMismatch {
        block: BasicBlockId,
        location: OperandLocation,
        expected: ScalarType,
        actual: ScalarType,
    },
    AssignmentScalarMismatch {
        block: BasicBlockId,
        statement: usize,
        expected: ScalarType,
        actual: ScalarType,
    },
    MissingTarget {
        block: BasicBlockId,
        target: BasicBlockId,
    },
    MissingCallDestination {
        block: BasicBlockId,
        local: LocalId,
    },
    NonBooleanCondition {
        block: BasicBlockId,
        actual: ScalarType,
    },
    PropagationFromPlainBody {
        block: BasicBlockId,
    },
    InvalidLoopCondition {
        header: BasicBlockId,
        condition: BasicBlockId,
    },
    DuplicateLoopHeader {
        header: BasicBlockId,
    },
    InvalidLoopHeaderPath {
        header: BasicBlockId,
        condition: BasicBlockId,
    },
    InvalidLoopContinuePath {
        header: BasicBlockId,
        continue_target: BasicBlockId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperandLocation {
    Statement(usize),
    CallArgument(usize),
}

pub(crate) fn validate(body: &Body) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    if body.locals.get(body.return_local.index()).is_none() {
        errors.push(ValidationError::MissingReturnLocal(body.return_local));
    }
    if body.blocks.get(body.entry.index()).is_none() {
        errors.push(ValidationError::MissingEntryBlock(body.entry));
    }
    let mut loop_headers = std::collections::HashSet::new();
    for region in &body.loop_regions {
        if !loop_headers.insert(region.header) {
            errors.push(ValidationError::DuplicateLoopHeader {
                header: region.header,
            });
        }
        for target in [
            region.header,
            region.condition,
            region.body,
            region.continue_target,
            region.exit,
        ] {
            if body.blocks.get(target.index()).is_none() {
                errors.push(ValidationError::MissingTarget {
                    block: region.header,
                    target,
                });
            }
        }
        if !matches!(
            body.blocks.get(region.condition.index()).map(|block| &block.terminator),
            Some(Terminator::Switch {
                then_target,
                else_target,
                ..
            }) if *then_target == region.body && *else_target == region.exit
        ) {
            errors.push(ValidationError::InvalidLoopCondition {
                header: region.header,
                condition: region.condition,
            });
        }
        if !linear_path_reaches(body, region.header, region.condition) {
            errors.push(ValidationError::InvalidLoopHeaderPath {
                header: region.header,
                condition: region.condition,
            });
        }
        if region.continue_target != region.header
            && !linear_path_reaches(body, region.continue_target, region.header)
        {
            errors.push(ValidationError::InvalidLoopContinuePath {
                header: region.header,
                continue_target: region.continue_target,
            });
        }
    }

    for (block_index, block) in body.blocks.iter().enumerate() {
        let block_id = BasicBlockId::from_index(block_index);
        for (statement_index, statement) in block.statements.iter().enumerate() {
            let Statement::Assign {
                destination, value, ..
            } = statement;
            let Some(destination_local) = body.locals.get(destination.local.index()) else {
                errors.push(ValidationError::MissingAssignmentLocal {
                    block: block_id,
                    statement: statement_index,
                    local: destination.local,
                });
                continue;
            };
            let value_ty = match value {
                super::model::Rvalue::Use(operand) => {
                    let ty = operand_type(
                        body,
                        block_id,
                        OperandLocation::Statement(statement_index),
                        operand,
                        &mut errors,
                    );
                    validate_operand_scalar(
                        body,
                        block_id,
                        OperandLocation::Statement(statement_index),
                        operand,
                        destination_local.scalar,
                        &mut errors,
                    );
                    ty
                }
                super::model::Rvalue::Binary {
                    left, right, ty, ..
                } => {
                    for operand in [left, right] {
                        validate_operand(
                            body,
                            block_id,
                            OperandLocation::Statement(statement_index),
                            operand,
                            *ty,
                            destination_local.scalar,
                            &mut errors,
                        );
                    }
                    Some(*ty)
                }
                super::model::Rvalue::Compare {
                    left,
                    right,
                    operand_ty,
                    operand_scalar,
                    result_ty,
                    ..
                } => {
                    for operand in [left, right] {
                        validate_operand(
                            body,
                            block_id,
                            OperandLocation::Statement(statement_index),
                            operand,
                            *operand_ty,
                            *operand_scalar,
                            &mut errors,
                        );
                    }
                    if destination_local.scalar != ScalarType::Bool {
                        errors.push(ValidationError::AssignmentScalarMismatch {
                            block: block_id,
                            statement: statement_index,
                            expected: ScalarType::Bool,
                            actual: destination_local.scalar,
                        });
                    }
                    Some(*result_ty)
                }
            };
            if value_ty.is_some_and(|value_ty| destination_local.ty != value_ty) {
                errors.push(ValidationError::AssignmentTypeMismatch {
                    block: block_id,
                    statement: statement_index,
                    destination: destination_local.ty,
                    value: value_ty.unwrap(),
                });
            }
        }

        match &block.terminator {
            Terminator::Goto { target } => {
                validate_target(body, block_id, *target, &mut errors);
            }
            Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => {
                validate_target(body, block_id, *then_target, &mut errors);
                validate_target(body, block_id, *else_target, &mut errors);
                if let Some(actual) = operand_scalar(body, condition)
                    && actual != ScalarType::Bool
                {
                    errors.push(ValidationError::NonBooleanCondition {
                        block: block_id,
                        actual,
                    });
                }
            }
            Terminator::Call {
                arguments,
                continuation,
                ..
            } => {
                let destination = match continuation {
                    CallContinuation::Return {
                        destination,
                        target,
                    } => {
                        validate_target(body, block_id, *target, &mut errors);
                        Some(destination)
                    }
                    CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                    } => {
                        validate_target(body, block_id, *success, &mut errors);
                        validate_target(body, block_id, *failure, &mut errors);
                        Some(destination)
                    }
                    CallContinuation::Never => None,
                };
                if let Some(destination) = destination
                    && body.locals.get(destination.local.index()).is_none()
                {
                    errors.push(ValidationError::MissingCallDestination {
                        block: block_id,
                        local: destination.local,
                    });
                }
                for (index, argument) in arguments.iter().enumerate() {
                    validate_operand(
                        body,
                        block_id,
                        OperandLocation::CallArgument(index),
                        &argument.operand,
                        argument.ty,
                        argument.scalar,
                        &mut errors,
                    );
                }
            }
            Terminator::PropagateFailure if body.return_mode == super::ReturnMode::Plain => {
                errors.push(ValidationError::PropagationFromPlainBody { block: block_id });
            }
            Terminator::Trap | Terminator::PropagateFailure | Terminator::Return => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn linear_path_reaches(body: &Body, start: BasicBlockId, destination: BasicBlockId) -> bool {
    let mut current = start;
    let mut visited = std::collections::HashSet::new();
    loop {
        if current == destination {
            return true;
        }
        if !visited.insert(current) {
            return false;
        }
        let Some(block) = body.blocks.get(current.index()) else {
            return false;
        };
        current = match &block.terminator {
            Terminator::Goto { target } => *target,
            Terminator::Call {
                continuation: CallContinuation::Return { target, .. },
                ..
            } => *target,
            Terminator::Call {
                continuation:
                    CallContinuation::Outcome {
                        success, failure, ..
                    },
                ..
            } if body.blocks.get(failure.index()).is_some_and(|failure| {
                failure.statements.is_empty()
                    && matches!(
                        failure.terminator,
                        Terminator::Trap | Terminator::PropagateFailure
                    )
            }) =>
            {
                *success
            }
            _ => return false,
        };
    }
}

fn validate_operand(
    body: &Body,
    block: BasicBlockId,
    location: OperandLocation,
    operand: &Operand,
    expected_ty: TyId,
    expected_scalar: ScalarType,
    errors: &mut Vec<ValidationError>,
) -> Option<TyId> {
    let actual_ty = operand_type(body, block, location, operand, errors);
    if let Some(actual) = actual_ty
        && actual != expected_ty
    {
        errors.push(ValidationError::OperandTypeMismatch {
            block,
            location,
            expected: expected_ty,
            actual,
        });
    }
    validate_operand_scalar(body, block, location, operand, expected_scalar, errors);
    actual_ty
}

fn validate_operand_scalar(
    body: &Body,
    block: BasicBlockId,
    location: OperandLocation,
    operand: &Operand,
    expected: ScalarType,
    errors: &mut Vec<ValidationError>,
) {
    if let Some(actual) = operand_scalar(body, operand)
        && actual != expected
    {
        errors.push(ValidationError::OperandScalarMismatch {
            block,
            location,
            expected,
            actual,
        });
    }
}

fn validate_target(
    body: &Body,
    block: BasicBlockId,
    target: BasicBlockId,
    errors: &mut Vec<ValidationError>,
) {
    if body.blocks.get(target.index()).is_none() {
        errors.push(ValidationError::MissingTarget { block, target });
    }
}

fn operand_scalar(body: &Body, operand: &Operand) -> Option<ScalarType> {
    match operand {
        Operand::Constant(constant) => Some(constant.scalar),
        Operand::Copy(place) => body
            .locals
            .get(place.local.index())
            .map(|local| local.scalar),
    }
}

fn operand_type(
    body: &Body,
    block: BasicBlockId,
    location: OperandLocation,
    operand: &Operand,
    errors: &mut Vec<ValidationError>,
) -> Option<TyId> {
    match operand {
        Operand::Constant(constant) => Some(constant.ty),
        Operand::Copy(place) => match body.locals.get(place.local.index()) {
            Some(local) => Some(local.ty),
            None => {
                errors.push(ValidationError::MissingOperandLocal {
                    block,
                    location,
                    local: place.local,
                });
                None
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::model::{
        BasicBlock, CallArgument, Constant, Local, LocalSource, Operand, Place, Rvalue,
    };
    use crate::semantic::{BodyId, DefId, ExprId};
    use crate::source::{ByteSpan, SourceId};

    fn span() -> ByteSpan {
        ByteSpan::new(SourceId::new(0), 0, 1)
    }

    fn valid_body() -> Body {
        let ty = TyId::from_index(0);
        Body {
            source_body: BodyId::from_index(0),
            source_span: span(),
            return_local: LocalId::from_index(0),
            return_mode: crate::mir::ReturnMode::Plain,
            locals: vec![Local {
                ty,
                scalar: crate::mir::model::ScalarType::I32,
                source: LocalSource::Return,
            }],
            entry: BasicBlockId::from_index(0),
            blocks: vec![
                BasicBlock {
                    statements: vec![Statement::Assign {
                        destination: Place {
                            local: LocalId::from_index(0),
                        },
                        value: Rvalue::Use(Operand::Constant(Constant {
                            ty,
                            scalar: crate::mir::model::ScalarType::I32,
                            value: 7,
                        })),
                        origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
                    }],
                    terminator: Terminator::Goto {
                        target: BasicBlockId::from_index(1),
                    },
                },
                BasicBlock {
                    statements: Vec::new(),
                    terminator: Terminator::Return,
                },
            ],
            loop_regions: Vec::new(),
        }
    }

    #[test]
    fn accepts_a_well_formed_body() {
        assert_eq!(validate(&valid_body()), Ok(()));
    }

    #[test]
    fn rejects_failure_propagation_from_a_plain_body() {
        let mut body = valid_body();
        body.blocks[1].terminator = Terminator::PropagateFailure;

        assert_eq!(
            validate(&body),
            Err(vec![ValidationError::PropagationFromPlainBody {
                block: BasicBlockId::from_index(1),
            }])
        );

        body.return_mode = crate::mir::ReturnMode::Fallible;
        assert_eq!(validate(&body), Ok(()));
    }

    #[test]
    fn reports_identity_and_type_errors_together() {
        let mut body = valid_body();
        body.return_local = LocalId::from_index(4);
        body.blocks[0].statements[0] = Statement::Assign {
            destination: Place {
                local: LocalId::from_index(0),
            },
            value: Rvalue::Use(Operand::Constant(Constant {
                ty: TyId::from_index(1),
                scalar: crate::mir::model::ScalarType::I32,
                value: 7,
            })),
            origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
        };
        body.blocks[0].terminator = Terminator::Goto {
            target: BasicBlockId::from_index(8),
        };

        assert_eq!(
            validate(&body),
            Err(vec![
                ValidationError::MissingReturnLocal(LocalId::from_index(4)),
                ValidationError::AssignmentTypeMismatch {
                    block: BasicBlockId::from_index(0),
                    statement: 0,
                    destination: TyId::from_index(0),
                    value: TyId::from_index(1),
                },
                ValidationError::MissingTarget {
                    block: BasicBlockId::from_index(0),
                    target: BasicBlockId::from_index(8),
                },
            ])
        );
    }

    #[test]
    fn reports_call_edge_and_argument_invariants_together() {
        let mut body = valid_body();
        body.blocks[0].statements.clear();
        body.blocks[0].terminator = Terminator::Call {
            origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
            callee: DefId::from_index(0),
            arguments: vec![CallArgument {
                operand: Operand::Constant(Constant {
                    ty: TyId::from_index(1),
                    scalar: ScalarType::Usize,
                    value: 7,
                }),
                ty: TyId::from_index(0),
                scalar: ScalarType::I32,
            }],
            continuation: CallContinuation::Return {
                destination: Place {
                    local: LocalId::from_index(4),
                },
                target: BasicBlockId::from_index(8),
            },
        };

        assert_eq!(
            validate(&body),
            Err(vec![
                ValidationError::MissingTarget {
                    block: BasicBlockId::from_index(0),
                    target: BasicBlockId::from_index(8),
                },
                ValidationError::MissingCallDestination {
                    block: BasicBlockId::from_index(0),
                    local: LocalId::from_index(4),
                },
                ValidationError::OperandTypeMismatch {
                    block: BasicBlockId::from_index(0),
                    location: OperandLocation::CallArgument(0),
                    expected: TyId::from_index(0),
                    actual: TyId::from_index(1),
                },
                ValidationError::OperandScalarMismatch {
                    block: BasicBlockId::from_index(0),
                    location: OperandLocation::CallArgument(0),
                    expected: ScalarType::I32,
                    actual: ScalarType::Usize,
                },
            ])
        );
    }
}
