use super::*;
use crate::ast::ConformanceMember;

pub(super) fn recovery_completion_context_at_offset(
    ast: &AstFile,
    offset: usize,
) -> Option<RecoveryCompletionContext<'_>> {
    ast.items
        .iter()
        .find_map(|item| completion_context_in_item_at_offset(item, offset))
}

fn completion_context_in_item_at_offset(
    item: &Item,
    offset: usize,
) -> Option<RecoveryCompletionContext<'_>> {
    match item {
        Item::Function(function) => function
            .body
            .as_ref()
            .and_then(|body| completion_context_in_block_at_offset(body, offset)),
        Item::Test(test) => completion_context_in_block_at_offset(&test.body, offset),
        Item::Instance(instance) => instance.callables().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| completion_context_in_block_at_offset(body, offset))
        }),
        Item::Destruct(destruct) => completion_context_in_block_at_offset(&destruct.body, offset),
        Item::Conformance(conformance) => {
            conformance.members.iter().find_map(|member| match member {
                ConformanceMember::AssociatedType(_) => None,
                ConformanceMember::Method(method) => method
                    .body
                    .as_ref()
                    .and_then(|body| completion_context_in_block_at_offset(body, offset)),
            })
        }
        Item::Interface(interface) => interface.methods.iter().find_map(|method| {
            method
                .body
                .as_ref()
                .and_then(|body| completion_context_in_block_at_offset(body, offset))
        }),
        Item::Construct(construct) => construct
            .functions()
            .find_map(|(_, function)| {
                function
                    .body
                    .as_ref()
                    .and_then(|body| completion_context_in_block_at_offset(body, offset))
            })
            .or_else(|| {
                construct.literals().find_map(|(_, literal)| {
                    literal
                        .body
                        .as_ref()
                        .and_then(|body| completion_context_in_block_at_offset(body, offset))
                })
            }),
        Item::Import(_)
        | Item::FromImport(_)
        | Item::Primitive(_)
        | Item::TypeAlias(_)
        | Item::Struct(_)
        | Item::Enum(_) => None,
    }
}

fn completion_context_in_block_at_offset(
    block: &Block,
    offset: usize,
) -> Option<RecoveryCompletionContext<'_>> {
    block
        .statements
        .iter()
        .find_map(|statement| completion_context_in_statement_at_offset(statement, offset))
        .or_else(|| {
            block
                .result
                .as_ref()
                .and_then(|result| completion_context_in_expression_at_offset(result, offset))
        })
}

fn completion_context_in_statement_at_offset(
    statement: &Stmt,
    offset: usize,
) -> Option<RecoveryCompletionContext<'_>> {
    match statement {
        Stmt::Return(statement) => statement
            .expression
            .as_ref()
            .and_then(|expression| completion_context_in_expression_at_offset(expression, offset)),
        Stmt::Binding(statement) => {
            completion_context_in_expression_at_offset(&statement.initializer, offset)
        }
        Stmt::Assignment(statement) => {
            completion_context_in_expression_at_offset(&statement.target, offset)
                .or_else(|| completion_context_in_expression_at_offset(&statement.value, offset))
        }
        Stmt::If(statement) => {
            completion_context_in_expression_at_offset(&statement.condition, offset)
                .or_else(|| completion_context_in_block_at_offset(&statement.then_block, offset))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| completion_context_in_block_at_offset(block, offset))
                })
        }
        Stmt::IfIs(statement) => {
            enum_pattern_completion_context_in_if_is_at_offset(statement, offset)
                .or_else(|| {
                    completion_context_in_expression_at_offset(&statement.expression, offset)
                })
                .or_else(|| completion_context_in_block_at_offset(&statement.then_block, offset))
                .or_else(|| {
                    statement
                        .else_block
                        .as_ref()
                        .and_then(|block| completion_context_in_block_at_offset(block, offset))
                })
        }
        Stmt::Switch(statement) => completion_context_in_switch_at_offset(statement, offset)
            .or_else(|| completion_context_in_expression_at_offset(&statement.expression, offset)),
        Stmt::ForRange(statement) => {
            completion_context_in_expression_at_offset(&statement.start, offset)
                .or_else(|| completion_context_in_expression_at_offset(&statement.end, offset))
                .or_else(|| completion_context_in_block_at_offset(&statement.body, offset))
        }
        Stmt::CollectionFor(statement) => {
            completion_context_in_expression_at_offset(&statement.source, offset)
                .or_else(|| completion_context_in_block_at_offset(&statement.body, offset))
        }
        Stmt::LiteralPackFor(statement) => {
            completion_context_in_block_at_offset(&statement.body, offset)
        }
        Stmt::While(statement) => {
            completion_context_in_expression_at_offset(&statement.condition, offset)
                .or_else(|| completion_context_in_block_at_offset(&statement.body, offset))
        }
        Stmt::Loop(statement) => completion_context_in_block_at_offset(&statement.body, offset),
        Stmt::Region(statement) => {
            completion_context_in_expression_at_offset(&statement.allocator, offset)
                .or_else(|| {
                    cursor_touches_span(statement.allocator.span(), offset)
                        .then_some(RecoveryCompletionContext::RegionAllocator)
                })
                .or_else(|| completion_context_in_block_at_offset(&statement.body, offset))
        }
        Stmt::Expression(statement) => {
            completion_context_in_expression_at_offset(&statement.expression, offset)
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Drop(_) => None,
    }
}

fn cursor_touches_span(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}

fn completion_context_in_expression_at_offset(
    expression: &Expr,
    offset: usize,
) -> Option<RecoveryCompletionContext<'_>> {
    match expression {
        Expr::Closure(expression) => {
            completion_context_in_block_at_offset(&expression.body, offset)
        }
        Expr::InterpolatedString(expression) => {
            expression.parts.iter().find_map(|part| match part {
                crate::ast::InterpolatedStringPart::Expression(part) => {
                    completion_context_in_expression_at_offset(&part.expression, offset)
                }
                crate::ast::InterpolatedStringPart::Text(_) => None,
            })
        }
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .find_map(|element| completion_context_in_expression_at_offset(element, offset)),
        Expr::TypedSequenceLiteral(expression) => (expression.target.span().end <= offset)
            .then_some(RecoveryCompletionContext::LiteralShape(&expression.target))
            .filter(|_| offset <= expression.elements_span.start)
            .or_else(|| {
                expression
                    .elements
                    .iter()
                    .find_map(|element| completion_context_in_expression_at_offset(element, offset))
            })
            .or_else(|| {
                expression.using.as_ref().and_then(|using| {
                    completion_context_in_expression_at_offset(&using.allocator, offset)
                })
            }),
        Expr::TypedStringLiteral(expression) => (expression.target.span().end <= offset
            && offset <= expression.text.span.start)
            .then_some(RecoveryCompletionContext::LiteralShape(&expression.target))
            .or_else(|| {
                expression.using.as_ref().and_then(|using| {
                    completion_context_in_expression_at_offset(&using.allocator, offset)
                })
            }),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .find_map(|field| completion_context_in_expression_at_offset(&field.value, offset))
            .or_else(|| {
                struct_literal_field_recovery_completion_context_at_offset(expression, offset)
            }),
        Expr::Propagate(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Force(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Catch(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
                .or_else(|| completion_context_in_block_at_offset(&expression.catch_block, offset))
        }
        Expr::Borrow(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Unary(expression) => {
            completion_context_in_expression_at_offset(&expression.operand, offset)
        }
        Expr::Binary(expression) => {
            completion_context_in_expression_at_offset(&expression.left, offset)
                .or_else(|| completion_context_in_expression_at_offset(&expression.right, offset))
        }
        Expr::TypeConversion(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Call(expression) => {
            completion_context_in_expression_at_offset(&expression.callee, offset).or_else(|| {
                expression.arguments.iter().find_map(|argument| {
                    completion_context_in_expression_at_offset(argument, offset)
                })
            })
        }
        Expr::Member(expression) => {
            member_completion_context_in_member_expression_at_offset(expression, offset)
                .or_else(|| completion_context_in_expression_at_offset(&expression.object, offset))
        }
        Expr::Index(expression) => {
            completion_context_in_expression_at_offset(&expression.object, offset)
                .or_else(|| completion_context_in_expression_at_offset(&expression.index, offset))
        }
        Expr::Group(expression) => {
            completion_context_in_expression_at_offset(&expression.expression, offset)
        }
        Expr::Otherwise(expression) => {
            completion_context_in_expression_at_offset(&expression.value, offset)
                .or_else(|| completion_context_in_block_at_offset(&expression.fallback, offset))
        }
        Expr::If(expression) => {
            completion_context_in_expression_at_offset(&expression.condition, offset)
                .or_else(|| completion_context_in_block_at_offset(&expression.then_block, offset))
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| completion_context_in_block_at_offset(block, offset))
                })
        }
        Expr::IfIs(expression) => {
            enum_pattern_completion_context_in_if_is_at_offset(expression, offset)
                .or_else(|| {
                    completion_context_in_expression_at_offset(&expression.expression, offset)
                })
                .or_else(|| completion_context_in_block_at_offset(&expression.then_block, offset))
                .or_else(|| {
                    expression
                        .else_block
                        .as_ref()
                        .and_then(|block| completion_context_in_block_at_offset(block, offset))
                })
        }
        Expr::Match(expression) => completion_context_in_switch_at_offset(expression, offset)
            .or_else(|| completion_context_in_expression_at_offset(&expression.expression, offset)),
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => None,
    }
}

fn enum_pattern_completion_context_in_if_is_at_offset(
    statement: &IfIsStmt,
    offset: usize,
) -> Option<RecoveryCompletionContext<'_>> {
    offset_in_member_completion(
        statement.enum_name_span,
        statement.variant_name_span,
        offset,
    )
    .then_some(RecoveryCompletionContext::EnumPatternMembers(
        statement.enum_name.as_str(),
    ))
}

fn completion_context_in_switch_at_offset(
    statement: &SwitchStmt,
    offset: usize,
) -> Option<RecoveryCompletionContext<'_>> {
    statement
        .arms
        .iter()
        .find_map(|arm| enum_pattern_completion_context_in_switch_arm_at_offset(arm, offset))
        .or_else(|| {
            statement
                .arms
                .iter()
                .find_map(|arm| completion_context_in_block_at_offset(&arm.body, offset))
        })
        .or_else(|| {
            statement
                .wildcard_arm
                .as_ref()
                .and_then(|arm| completion_context_in_block_at_offset(&arm.body, offset))
        })
}

fn enum_pattern_completion_context_in_switch_arm_at_offset(
    arm: &SwitchArm,
    offset: usize,
) -> Option<RecoveryCompletionContext<'_>> {
    offset_in_member_completion(arm.enum_name_span, arm.variant_name_span, offset).then_some(
        RecoveryCompletionContext::EnumPatternMembers(arm.enum_name.as_str()),
    )
}

fn member_completion_context_in_member_expression_at_offset(
    expression: &MemberExpr,
    offset: usize,
) -> Option<RecoveryCompletionContext<'_>> {
    let Expr::Identifier(owner) = expression.object.without_groups() else {
        return None;
    };

    offset_in_member_completion(owner.span, expression.member_span, offset).then_some(
        RecoveryCompletionContext::MemberAccess {
            owner_name: owner.name.as_str(),
            owner_span: owner.span,
        },
    )
}

fn struct_literal_field_recovery_completion_context_at_offset(
    literal: &StructLiteralExpr,
    offset: usize,
) -> Option<RecoveryCompletionContext<'_>> {
    if !span_contains(literal.fields_span, offset) {
        return None;
    }
    if literal
        .fields
        .iter()
        .any(|field| span_contains(field.value.span(), offset))
    {
        return None;
    }

    Some(RecoveryCompletionContext::StructLiteralFields { literal, offset })
}

fn offset_in_member_completion(owner_span: ByteSpan, member_span: ByteSpan, offset: usize) -> bool {
    owner_span.source == member_span.source && owner_span.end < offset && offset <= member_span.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::completion_sites::CompletionSiteKind;
    use crate::analysis::test_support::analyze_text;

    #[test]
    fn indexed_sites_match_the_complete_recovery_walk_at_every_offset() {
        let text = r#"struct Point { pub x: i32, pub y: i32 }

enum Choice { hit miss }

struct Allocator { state: usize }

func main(point: Point, choice: Choice, allocator: Allocator): i32 {
    let values = Vec<i32> [1, 2]
    let text = String "hello"
    let made = Point { x: point.x, y: 2 }
    region temp using allocator {
        if choice is Choice.hit {
            return point.x
        }
    }
    return match choice {
        Choice.hit { made.x }
        Choice.miss { 0 }
    }
}
"#;
        let (_, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("root file");

        for offset in 0..=text.len() {
            assert_eq!(
                normalize_indexed(file.completion_sites.at_offset(offset)),
                normalize_recovery(recovery_completion_context_at_offset(&file.ast, offset)),
                "completion context mismatch at {offset}"
            );
        }
    }

    fn normalize_indexed(context: Option<&CompletionSiteKind>) -> Option<String> {
        Some(match context? {
            CompletionSiteKind::LiteralShape(target) => {
                format!("literal:{}", crate::ast::canonical_type_expr(target))
            }
            CompletionSiteKind::RegionAllocator => "region".to_string(),
            CompletionSiteKind::EnumPatternMembers(owner) => format!("enum:{owner}"),
            CompletionSiteKind::MemberAccess {
                owner_name,
                owner_span,
            } => format!("member:{owner_name}:{owner_span:?}"),
            CompletionSiteKind::StructLiteralFields(literal) => {
                format!("struct:{:?}", literal.span)
            }
        })
    }

    fn normalize_recovery(context: Option<RecoveryCompletionContext<'_>>) -> Option<String> {
        Some(match context? {
            RecoveryCompletionContext::LiteralShape(target) => {
                format!("literal:{}", crate::ast::canonical_type_expr(target))
            }
            RecoveryCompletionContext::RegionAllocator => "region".to_string(),
            RecoveryCompletionContext::EnumPatternMembers(owner) => format!("enum:{owner}"),
            RecoveryCompletionContext::MemberAccess {
                owner_name,
                owner_span,
            } => format!("member:{owner_name}:{owner_span:?}"),
            RecoveryCompletionContext::StructLiteralFields { literal, .. } => {
                format!("struct:{:?}", literal.span)
            }
        })
    }
}
