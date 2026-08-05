use super::Formatter;
use crate::ast::{DirectiveValue, PackageDirective};

impl Formatter {
    pub(super) fn format_package_directive(&mut self, directive: &PackageDirective) {
        self.write("#");
        self.write(&directive.name);
        self.write(": ");
        self.format_directive_value(&directive.value);
    }

    fn format_directive_value(&mut self, value: &DirectiveValue) {
        match value {
            DirectiveValue::String { value, .. } => self.write_quoted_string_literal(value),
            DirectiveValue::Integer { value, .. } => self.write(&value.to_string()),
            DirectiveValue::Boolean { value, .. } => {
                self.write(if *value { "true" } else { "false" });
            }
            DirectiveValue::List { values, .. } => {
                self.write("[");
                self.write_comma_separated(values, Self::format_directive_value);
                self.write("]");
            }
            DirectiveValue::Record { fields, .. } => {
                if fields.is_empty() {
                    self.write("{}");
                    return;
                }
                self.write("{");
                self.newline();
                self.indented(|formatter| {
                    for field in fields {
                        formatter.write_indent();
                        formatter.write(&field.name);
                        formatter.write(": ");
                        formatter.format_directive_value(&field.value);
                        formatter.write(",");
                        formatter.newline();
                    }
                });
                self.write_indent();
                self.write("}");
            }
        }
    }
}
