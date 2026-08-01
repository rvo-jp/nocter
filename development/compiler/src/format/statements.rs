use super::Formatter;
use crate::ast::{
    AssignmentOperator, BindingKind, BindingStmt, Block, Expr, IfIsStmt, IfStmt, Stmt,
    SwitchPayloadPattern, SwitchStmt,
};

impl Formatter {
    pub(super) fn format_block(&mut self, block: &Block) {
        if block.statements.is_empty() && block.result.is_none() {
            self.write("{}");
            return;
        }

        if block.statements.is_empty()
            && let Some(result) = &block.result
            && expression_can_be_inline_block_result(result)
        {
            self.write("{ ");
            self.format_expression(result);
            self.write(" }");
            return;
        }

        self.write("{");
        self.newline();
        self.indented(|formatter| {
            for statement in &block.statements {
                formatter.write_indent();
                formatter.format_statement(statement);
                formatter.newline();
            }
            if let Some(result) = &block.result {
                formatter.write_indent();
                formatter.format_expression(result);
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Import(statement) => self.format_import_item(statement),
            Stmt::FromImport(statement) => self.format_from_import_item(statement),
            Stmt::Return(statement) => {
                self.write("return");
                if let Some(expression) = &statement.expression {
                    self.write(" ");
                    self.format_expression(expression);
                }
            }
            Stmt::Binding(statement) => self.format_binding_statement(statement),
            Stmt::Assignment(statement) => {
                self.format_expression(&statement.target);
                self.write(" ");
                self.write(assignment_operator_text(statement.operator));
                self.write(" ");
                self.format_expression(&statement.value);
            }
            Stmt::If(statement) => self.format_if_statement(statement),
            Stmt::IfIs(statement) => self.format_if_is_statement(statement),
            Stmt::Switch(statement) => self.format_switch_statement(statement),
            Stmt::ForRange(statement) => {
                self.write("for ");
                self.write(&statement.name);
                self.write(" in ");
                self.format_expression(&statement.start);
                self.write("..<");
                self.format_expression(&statement.end);
                self.write(" ");
                self.format_block(&statement.body);
            }
            Stmt::While(statement) => {
                self.write("while ");
                self.format_expression(&statement.condition);
                self.write(" ");
                self.format_block(&statement.body);
            }
            Stmt::Loop(statement) => {
                self.write("loop ");
                self.format_block(&statement.body);
            }
            Stmt::Region(statement) => {
                self.write("region ");
                self.write(&statement.name);
                self.write(" using ");
                self.format_expression(&statement.allocator);
                self.write(" ");
                self.format_block(&statement.body);
            }
            Stmt::Break(_) => self.write("break"),
            Stmt::Continue(_) => self.write("continue"),
            Stmt::Drop(statement) => {
                self.write("drop ");
                self.write(&statement.name);
            }
            Stmt::Expression(statement) => self.format_expression(&statement.expression),
        }
    }

    fn format_binding_statement(&mut self, statement: &BindingStmt) {
        self.format_binding_kind(statement.kind);
        self.write(" ");
        self.write(&statement.name);
        if let Some(ty) = &statement.ty {
            self.write(": ");
            self.format_type(ty);
        }
        self.write(" = ");
        self.format_expression(&statement.initializer);
    }

    pub(super) fn format_if_statement(&mut self, statement: &IfStmt) {
        self.write("if ");
        self.format_expression(&statement.condition);
        self.write(" ");
        self.format_block(&statement.then_block);
        self.format_else(&statement.else_block);
    }

    pub(super) fn format_if_is_statement(&mut self, statement: &IfIsStmt) {
        self.write("if ");
        self.format_expression(&statement.expression);
        self.write(" is ");
        self.format_if_is_pattern(statement);
        self.write(" ");
        self.format_block(&statement.then_block);
        self.format_else(&statement.else_block);
    }

    pub(super) fn format_switch_statement(&mut self, statement: &SwitchStmt) {
        self.write("match ");
        self.format_expression(&statement.expression);
        self.write(" ");

        if statement.arms.is_empty() && statement.wildcard_arm.is_none() {
            self.write("{}");
            return;
        }

        self.write("{");
        self.newline();
        self.indented(|formatter| {
            for arm in &statement.arms {
                formatter.write_indent();
                formatter.format_enum_pattern(
                    &arm.enum_name,
                    &arm.variant_name,
                    arm.payload.as_ref(),
                );
                formatter.write(" ");
                formatter.format_block(&arm.body);
                formatter.newline();
            }

            if let Some(wildcard_arm) = &statement.wildcard_arm {
                formatter.write_indent();
                formatter.write("_ ");
                formatter.format_block(&wildcard_arm.body);
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_else(&mut self, else_block: &Option<Block>) {
        let Some(else_block) = else_block else {
            return;
        };

        if else_block.statements.is_empty()
            && let Some(Expr::If(statement)) = else_block.result.as_deref()
        {
            self.write(" else ");
            self.format_if_statement(statement);
            return;
        }

        if else_block.statements.is_empty()
            && let Some(Expr::IfIs(statement)) = else_block.result.as_deref()
        {
            self.write(" else if ");
            self.format_expression(&statement.expression);
            self.write(" is ");
            self.format_if_is_pattern(statement);
            self.write(" ");
            self.format_block(&statement.then_block);
            self.format_else(&statement.else_block);
            return;
        }

        self.write(" else ");
        self.format_block(else_block);
    }

    fn format_if_is_pattern(&mut self, statement: &IfIsStmt) {
        self.write(&statement.enum_name);
        self.write(".");
        self.write(&statement.variant_name);
        if let Some(payload) = &statement.payload {
            self.write("(");
            self.format_payload_pattern(payload);
            self.write(")");
        }
    }

    fn format_enum_pattern(
        &mut self,
        enum_name: &str,
        variant_name: &str,
        payload: Option<&SwitchPayloadPattern>,
    ) {
        self.write(enum_name);
        self.write(".");
        self.write(variant_name);
        if let Some(payload) = payload {
            self.write("(");
            self.format_payload_pattern(payload);
            self.write(")");
        }
    }

    fn format_payload_pattern(&mut self, payload: &SwitchPayloadPattern) {
        match payload {
            SwitchPayloadPattern::Binding(binding) => self.write(&binding.name),
            SwitchPayloadPattern::Discard(_) => self.write("_"),
        }
    }

    fn format_binding_kind(&mut self, kind: BindingKind) {
        match kind {
            BindingKind::Let => self.write("let"),
            BindingKind::Var => self.write("var"),
        }
    }
}

fn assignment_operator_text(operator: AssignmentOperator) -> &'static str {
    match operator {
        AssignmentOperator::Assign => "=",
        AssignmentOperator::AddAssign => "+=",
        AssignmentOperator::SubtractAssign => "-=",
        AssignmentOperator::MultiplyAssign => "*=",
        AssignmentOperator::DivideAssign => "/=",
        AssignmentOperator::RemainderAssign => "%=",
    }
}

impl Formatter {
    fn format_expression(&mut self, expression: &Expr) {
        self.format_expr(expression, 0);
    }
}

fn expression_can_be_inline_block_result(expression: &Expr) -> bool {
    match expression {
        Expr::If(_) | Expr::IfIs(_) | Expr::Match(_) | Expr::Catch(_) | Expr::Otherwise(_) => false,
        Expr::ArrayLiteral(expression) => expression
            .elements
            .iter()
            .all(expression_can_be_inline_block_result),
        Expr::StructLiteral(expression) => expression
            .fields
            .iter()
            .all(|field| expression_can_be_inline_block_result(&field.value)),
        Expr::InterpolatedString(_)
        | Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::Propagate(_)
        | Expr::Force(_)
        | Expr::Borrow(_)
        | Expr::Unary(_)
        | Expr::Binary(_)
        | Expr::TypeConversion(_)
        | Expr::Call(_)
        | Expr::Member(_)
        | Expr::Index(_)
        | Expr::Group(_) => true,
    }
}
