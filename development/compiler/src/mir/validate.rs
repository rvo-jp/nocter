//! Structural MIR verification. Validation errors are compiler invariant
//! failures, not alternate source-language diagnostics.

use super::ids::{BasicBlockId, LocalId};
use super::model::{Body, Operand, ScalarType, Statement, Terminator};
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
        statement: usize,
        local: LocalId,
    },
    OperandTypeMismatch {
        block: BasicBlockId,
        statement: usize,
        expected: TyId,
        actual: TyId,
    },
    OperandScalarMismatch {
        block: BasicBlockId,
        statement: usize,
        expected: ScalarType,
        actual: ScalarType,
    },
    MissingTarget {
        block: BasicBlockId,
        target: BasicBlockId,
    },
    NonBooleanCondition {
        block: BasicBlockId,
        actual: ScalarType,
    },
}

pub(crate) fn validate(body: &Body) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    if body.locals.get(body.return_local.index()).is_none() {
        errors.push(ValidationError::MissingReturnLocal(body.return_local));
    }
    if body.blocks.get(body.entry.index()).is_none() {
        errors.push(ValidationError::MissingEntryBlock(body.entry));
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
                    operand_type(body, block_id, statement_index, operand, &mut errors)
                }
                super::model::Rvalue::Binary {
                    left, right, ty, ..
                } => {
                    for operand in [left, right] {
                        if let Some(actual) =
                            operand_type(body, block_id, statement_index, operand, &mut errors)
                            && actual != *ty
                        {
                            errors.push(ValidationError::OperandTypeMismatch {
                                block: block_id,
                                statement: statement_index,
                                expected: *ty,
                                actual,
                            });
                        }
                    }
                    Some(*ty)
                }
            };
            let operands = match value {
                super::model::Rvalue::Use(operand) => [Some(operand), None],
                super::model::Rvalue::Binary { left, right, .. } => [Some(left), Some(right)],
            };
            for operand in operands.into_iter().flatten() {
                let Operand::Copy(place) = operand else {
                    continue;
                };
                let Some(source_local) = body.locals.get(place.local.index()) else {
                    continue;
                };
                if source_local.scalar != destination_local.scalar {
                    errors.push(ValidationError::OperandScalarMismatch {
                        block: block_id,
                        statement: statement_index,
                        expected: destination_local.scalar,
                        actual: source_local.scalar,
                    });
                }
            }
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
            Terminator::Return => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
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
    statement: usize,
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
                    statement,
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
    use crate::mir::model::{BasicBlock, Constant, Local, LocalSource, Operand, Place, Rvalue};
    use crate::semantic::{BodyId, ExprId};
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
                        source: ExprId::from_index(0),
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
        }
    }

    #[test]
    fn accepts_a_well_formed_body() {
        assert_eq!(validate(&valid_body()), Ok(()));
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
            source: ExprId::from_index(0),
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
}
