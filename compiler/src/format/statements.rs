use super::Formatter;
use crate::ast::{
    BindingKind, BindingStmt, Block, Expr, IfLetStmt, IfStmt, Stmt, SwitchPayloadBinding,
    SwitchStmt,
};

impl Formatter {
    pub(super) fn format_block(&mut self, block: &Block) {
        if block.statements.is_empty() {
            self.write("{}");
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
        });
        self.write_indent();
        self.write("}");
    }

    fn format_statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Return(statement) => {
                self.write("return");
                if let Some(expression) = &statement.expression {
                    self.write(" ");
                    self.format_expression(expression);
                }
            }
            Stmt::Binding(statement) => self.format_binding_statement(statement),
            Stmt::If(statement) => self.format_if_statement(statement),
            Stmt::IfIs(statement) => {
                self.write("if ");
                self.format_expression(&statement.expression);
                self.write(" is ");
                self.format_enum_pattern(
                    &statement.enum_name,
                    &statement.variant_name,
                    statement.payload.as_ref(),
                );
                self.write(" ");
                self.format_block(&statement.then_block);
                self.format_else(&statement.else_block);
            }
            Stmt::IfLet(statement) => self.format_if_let_statement(statement),
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
            Stmt::WhileLet(statement) => {
                self.write("while ");
                self.format_binding_kind(statement.kind);
                self.write(" ");
                self.write(&statement.name);
                self.write(" = ");
                self.format_expression(&statement.initializer);
                self.write(" ");
                self.format_block(&statement.body);
            }
            Stmt::Loop(statement) => {
                self.write("loop ");
                self.format_block(&statement.body);
            }
            Stmt::Break(_) => self.write("break"),
            Stmt::Continue(_) => self.write("continue"),
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

        if let Some(else_block) = &statement.else_block {
            self.write(" else ");
            self.format_block(else_block);
        }
    }

    fn format_if_statement(&mut self, statement: &IfStmt) {
        self.write("if ");
        self.format_expression(&statement.condition);
        self.write(" ");
        self.format_block(&statement.then_block);
        self.format_else(&statement.else_block);
    }

    fn format_if_let_statement(&mut self, statement: &IfLetStmt) {
        self.write("if ");
        self.format_binding_kind(statement.kind);
        self.write(" ");
        self.write(&statement.name);
        self.write(" = ");
        self.format_expression(&statement.initializer);
        self.write(" ");
        self.format_block(&statement.then_block);
        self.format_else(&statement.else_block);
    }

    fn format_switch_statement(&mut self, statement: &SwitchStmt) {
        self.write("match ");
        self.format_expression(&statement.expression);
        self.write(" ");

        if statement.arms.is_empty() && statement.else_arm.is_none() {
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

            if let Some(else_arm) = &statement.else_arm {
                formatter.write_indent();
                formatter.write("else ");
                formatter.format_block(&else_arm.body);
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

        if let [Stmt::If(statement)] = else_block.statements.as_slice() {
            self.write(" else ");
            self.format_if_statement(statement);
            return;
        }

        if let [Stmt::IfIs(statement)] = else_block.statements.as_slice() {
            self.write(" else if ");
            self.format_expression(&statement.expression);
            self.write(" is ");
            self.format_enum_pattern(
                &statement.enum_name,
                &statement.variant_name,
                statement.payload.as_ref(),
            );
            self.write(" ");
            self.format_block(&statement.then_block);
            self.format_else(&statement.else_block);
            return;
        }

        if let [Stmt::IfLet(statement)] = else_block.statements.as_slice() {
            self.write(" else ");
            self.format_if_let_statement(statement);
            return;
        }

        self.write(" else ");
        self.format_block(else_block);
    }

    fn format_enum_pattern(
        &mut self,
        enum_name: &str,
        variant_name: &str,
        payload: Option<&SwitchPayloadBinding>,
    ) {
        self.write(enum_name);
        self.write(".");
        self.write(variant_name);
        if let Some(payload) = payload {
            self.write("(");
            self.write(&payload.name);
            self.write(")");
        }
    }

    fn format_binding_kind(&mut self, kind: BindingKind) {
        match kind {
            BindingKind::Let => self.write("let"),
            BindingKind::Var => self.write("var"),
        }
    }
}

impl Formatter {
    fn format_expression(&mut self, expression: &Expr) {
        self.format_expr(expression, 0);
    }
}
