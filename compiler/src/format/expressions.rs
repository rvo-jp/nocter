use super::Formatter;
use crate::ast::{
    BinaryOperator, Expr, PatternConditionalArm, PatternConditionalExpr, StructLiteralExpr,
    StructLiteralField, SwitchPayloadBinding, UnaryOperator,
};

const PREC_OPTIONAL_DEFAULT: u8 = 1;
const PREC_LOGICAL_OR: u8 = 2;
const PREC_LOGICAL_AND: u8 = 3;
const PREC_EQUALITY: u8 = 4;
const PREC_ORDERING: u8 = 5;
const PREC_SHIFT: u8 = 6;
const PREC_ADDITIVE: u8 = 7;
const PREC_MULTIPLICATIVE: u8 = 8;
const PREC_PREFIX: u8 = 9;
const PREC_POSTFIX: u8 = 10;
const PREC_PRIMARY: u8 = 11;

impl Formatter {
    pub(super) fn format_expr(&mut self, expression: &Expr, parent_precedence: u8) {
        let precedence = expression_precedence(expression);
        let needs_group = precedence < parent_precedence;

        if needs_group {
            self.write("(");
        }

        match expression {
            Expr::Identifier(expression) => self.write(&expression.name),
            Expr::IntegerLiteral(expression)
            | Expr::StringLiteral(expression)
            | Expr::BoolLiteral(expression)
            | Expr::NoneLiteral(expression) => self.write(&expression.value),
            Expr::InterpolatedString(expression) => self.write(&expression.value),
            Expr::ArrayLiteral(expression) => {
                self.write("[");
                self.write_comma_separated(&expression.elements, |formatter, element| {
                    formatter.format_expr(element, 0);
                });
                self.write("]");
            }
            Expr::StructLiteral(expression) => self.format_struct_literal(expression),
            Expr::Propagate(expression) => {
                self.format_expr(&expression.expression, PREC_POSTFIX);
                self.write("?");
            }
            Expr::Force(expression) => {
                self.format_expr(&expression.expression, PREC_POSTFIX);
                self.write("!");
            }
            Expr::Catch(expression) => {
                self.format_expr(&expression.expression, PREC_POSTFIX);
                self.write(" catch ");
                self.write(&expression.error_name);
                self.write(" ");
                self.format_block(&expression.catch_block);
            }
            Expr::Borrow(expression) => {
                self.write(if expression.is_readwrite { "&+" } else { "&" });
                self.format_expr(&expression.expression, PREC_PREFIX);
            }
            Expr::Unary(expression) => {
                self.write(unary_spelling(expression.operator));
                self.format_expr(&expression.operand, PREC_PREFIX);
            }
            Expr::Binary(expression) => {
                let precedence = binary_precedence(expression.operator);
                self.format_expr(&expression.left, precedence);
                self.write(" ");
                self.write(expression.operator.spelling());
                self.write(" ");
                self.format_expr(&expression.right, precedence + 1);
            }
            Expr::TypeConversion(expression) => {
                self.format_expr(&expression.expression, PREC_POSTFIX);
                self.write(" as ");
                self.format_type(&expression.ty);
            }
            Expr::Call(expression) => {
                self.format_expr(&expression.callee, PREC_POSTFIX);
                self.write("(");
                self.write_comma_separated(&expression.arguments, |formatter, argument| {
                    formatter.format_expr(argument, 0);
                });
                self.write(")");
            }
            Expr::Member(expression) => {
                self.format_expr(&expression.object, PREC_POSTFIX);
                self.write(".");
                self.write(&expression.member);
            }
            Expr::Index(expression) => {
                self.format_expr(&expression.object, PREC_POSTFIX);
                self.write("[");
                self.format_expr(&expression.index, 0);
                self.write("]");
            }
            Expr::Group(expression) => {
                self.write("(");
                self.format_expr(&expression.expression, 0);
                self.write(")");
            }
            Expr::OptionalDefault(expression) => {
                self.format_expr(&expression.value, PREC_OPTIONAL_DEFAULT + 1);
                self.write(" ?? ");
                self.format_expr(&expression.default, PREC_OPTIONAL_DEFAULT);
            }
            Expr::PatternConditional(expression) => {
                self.format_pattern_conditional(expression);
            }
        }

        if needs_group {
            self.write(")");
        }
    }

    fn format_struct_literal(&mut self, expression: &StructLiteralExpr) {
        self.format_type(&expression.ty);
        self.write(" {");
        if !expression.fields.is_empty() {
            self.write(" ");
            self.write_comma_separated(&expression.fields, Self::format_struct_literal_field);
            self.write(" ");
        }
        self.write("}");
    }

    fn format_struct_literal_field(&mut self, field: &StructLiteralField) {
        self.write(&field.name);
        self.write(": ");
        self.format_expr(&field.value, 0);
    }

    fn format_pattern_conditional(&mut self, expression: &PatternConditionalExpr) {
        self.format_expr(&expression.target, PREC_OPTIONAL_DEFAULT + 1);
        self.write(" ?{");
        self.indented(|formatter| {
            for arm in &expression.arms {
                formatter.newline();
                formatter.write_indent();
                formatter.format_pattern_conditional_arm(arm);
            }
            formatter.newline();
            formatter.write_indent();
            formatter.write(": ");
            formatter.format_expr(&expression.fallback, 0);
        });
        self.newline();
        self.write_indent();
        self.write("}");
    }

    fn format_pattern_conditional_arm(&mut self, arm: &PatternConditionalArm) {
        self.write(&arm.enum_name);
        self.write(".");
        self.write(&arm.variant_name);
        if let Some(payload) = &arm.payload {
            self.format_payload_binding(payload);
        }
        self.write(" : ");
        self.format_expr(&arm.expression, 0);
    }

    fn format_payload_binding(&mut self, payload: &SwitchPayloadBinding) {
        self.write("(");
        self.write(&payload.name);
        self.write(")");
    }
}

fn expression_precedence(expression: &Expr) -> u8 {
    match expression {
        Expr::OptionalDefault(_) | Expr::PatternConditional(_) => PREC_OPTIONAL_DEFAULT,
        Expr::Binary(expression) => binary_precedence(expression.operator),
        Expr::Borrow(_) | Expr::Unary(_) => PREC_PREFIX,
        Expr::Propagate(_)
        | Expr::Force(_)
        | Expr::Catch(_)
        | Expr::TypeConversion(_)
        | Expr::Call(_)
        | Expr::Member(_)
        | Expr::Index(_) => PREC_POSTFIX,
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::InterpolatedString(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::ArrayLiteral(_)
        | Expr::StructLiteral(_)
        | Expr::Group(_) => PREC_PRIMARY,
    }
}

fn binary_precedence(operator: BinaryOperator) -> u8 {
    match operator {
        BinaryOperator::LogicalOr => PREC_LOGICAL_OR,
        BinaryOperator::LogicalAnd => PREC_LOGICAL_AND,
        BinaryOperator::Equal | BinaryOperator::NotEqual => PREC_EQUALITY,
        BinaryOperator::Less
        | BinaryOperator::LessEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterEqual => PREC_ORDERING,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => PREC_SHIFT,
        BinaryOperator::Add | BinaryOperator::Subtract => PREC_ADDITIVE,
        BinaryOperator::Multiply | BinaryOperator::Divide | BinaryOperator::Remainder => {
            PREC_MULTIPLICATIVE
        }
    }
}

fn unary_spelling(operator: UnaryOperator) -> &'static str {
    match operator {
        UnaryOperator::LogicalNot => "!",
        UnaryOperator::Negate => "-",
    }
}
