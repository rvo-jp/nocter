use super::{
    AssignmentStmt, BindingStmt, BorrowExpr, Diagnostic, DiagnosticNote, NonCopyOwnedValueKind,
    SourceMap, Type, binding_keyword,
};

pub(in crate::typecheck) fn binding_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0342",
        format!(
            "`{keyword}` binding `{}` is annotated as `{}`, but the initializer has type `{}`",
            statement.name,
            expected.display(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    if let Some(annotation) = &statement.ty
        && let Ok(span) = sources.span_to_json(annotation.span())
    {
        diagnostic.notes.push(DiagnosticNote {
            message: format!("binding `{}` is annotated here", statement.name),
            span: Some(span),
        });
    }
    diagnostic.help = Some(format!(
        "change the initializer or annotate `{}` as `{}`",
        statement.name,
        actual.display()
    ));
    diagnostic
}

pub(in crate::typecheck) fn immutable_assignment_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    name: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0381",
        format!("cannot assign to immutable binding `{name}`"),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.target.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!("declare `{name}` with `var` to make it assignable"));
    diagnostic
}

pub(in crate::typecheck) fn non_writable_assignment_target_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0381", "assignment target is not writable");
    diagnostic.primary_span = sources
        .span_to_json(statement.target.span())
        .ok()
        .map(Box::new);
    diagnostic.help =
        Some("assign to a `var` binding, writable field, or `&+[...]` element".to_string());
    diagnostic
}

pub(in crate::typecheck) fn assignment_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    expected: &Type,
    actual: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0382",
        format!(
            "assignment target has type `{}`, but the assigned value has type `{}`",
            expected.display(),
            actual.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.value.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(format!(
        "change the assigned value to produce `{}`",
        expected.display()
    ));
    diagnostic
}

pub(in crate::typecheck) fn compound_assignment_operand_type_mismatch_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    target: &Type,
    value: &Type,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0437",
        format!(
            "operator `{}` combines assignment target `{}` with value `{}`, but compound assignment requires matching integer operands",
            assignment_operator_text(statement.operator),
            target.display(),
            value.display()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.operator_span)
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "use integer operands with the same type, or write a plain `=` assignment".to_string(),
    );
    diagnostic
}

pub(in crate::typecheck) fn non_copy_struct_assignment_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    source_name: &str,
    type_name: &str,
    kind: NonCopyOwnedValueKind,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0384",
        format!(
            "cannot implicitly copy {} `{type_name}` from `{source_name}`",
            kind.noun()
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.value.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(kind.copy_help(source_name, type_name));
    diagnostic
}

fn assignment_operator_text(operator: crate::ast::AssignmentOperator) -> &'static str {
    match operator {
        crate::ast::AssignmentOperator::Assign => "=",
        crate::ast::AssignmentOperator::AddAssign => "+=",
        crate::ast::AssignmentOperator::SubtractAssign => "-=",
        crate::ast::AssignmentOperator::MultiplyAssign => "*=",
        crate::ast::AssignmentOperator::DivideAssign => "/=",
        crate::ast::AssignmentOperator::RemainderAssign => "%=",
    }
}

pub(in crate::typecheck) fn non_copy_struct_binding_diagnostic(
    sources: &SourceMap,
    statement: &BindingStmt,
    source_name: &str,
    type_name: &str,
    kind: NonCopyOwnedValueKind,
) -> Diagnostic {
    let keyword = binding_keyword(statement.kind);
    let mut diagnostic = Diagnostic::error(
        "E0432",
        format!(
            "cannot implicitly copy {} `{type_name}` from `{source_name}` into `{keyword}` binding `{}`",
            kind.noun(),
            statement.name
        ),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.initializer.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(kind.copy_help(source_name, type_name));
    diagnostic
}

pub(in crate::typecheck) fn self_move_assignment_diagnostic(
    sources: &SourceMap,
    statement: &AssignmentStmt,
    name: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0395",
        format!("cannot assign `{name}` from `move {name}`"),
    );
    diagnostic.primary_span = sources
        .span_to_json(statement.value.span())
        .ok()
        .map(Box::new);
    diagnostic.help =
        Some("move from a different binding or construct a replacement value".to_string());
    diagnostic
}

pub(in crate::typecheck) fn readwrite_borrow_requires_writable_place_diagnostic(
    sources: &SourceMap,
    expression: &BorrowExpr,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0383",
        "`&+` requires a writable place, such as a `var` binding or writable aggregate field",
    );
    diagnostic.primary_span = sources
        .span_to_json(expression.expression.span())
        .ok()
        .map(Box::new);
    diagnostic.help = Some(
        "use `&` for a readonly borrow or borrow from a `var` binding or `&+` aggregate parameter"
            .to_string(),
    );
    diagnostic
}
