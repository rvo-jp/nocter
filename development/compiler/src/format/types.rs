use super::Formatter;
use crate::ast::TypeExpr;

impl Formatter {
    pub(super) fn format_type(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Closure(ty) => self.write(&ty.identity_name()),
            TypeExpr::Reference(ty) => self.write(&ty.name),
            TypeExpr::Generic(ty) => {
                self.write(&ty.name);
                self.write("<");
                self.write_comma_separated(&ty.arguments, Self::format_type);
                self.write(">");
            }
            TypeExpr::Pointer(ty) => {
                self.write("*");
                self.format_type(&ty.inner);
            }
            TypeExpr::Borrow(ty) => {
                if ty.is_readwrite {
                    self.write("&+");
                } else {
                    self.write("&");
                }
                self.format_type(&ty.inner);
            }
            TypeExpr::View(ty) => {
                if ty.is_readwrite {
                    self.write("&+");
                }
                self.write("[");
                self.format_type(&ty.element);
                self.write("]");
            }
            TypeExpr::Array(ty) => {
                self.write("[");
                self.format_type(&ty.element);
                self.write("; ");
                self.write(&ty.length.value);
                self.write("]");
            }
            TypeExpr::Optional(ty) => {
                self.format_type(&ty.inner);
                self.write("?");
            }
            TypeExpr::Fallible(ty) => {
                self.format_type(&ty.success);
                self.write("!");
            }
        }
    }
}
