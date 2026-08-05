use super::Formatter;
use crate::ast::TypeExpr;

impl Formatter {
    pub(super) fn format_type(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Callable(ty) => {
                self.write(ty.capability.source_prefix());
                self.write("func(");
                for (index, parameter) in ty.parameters.iter().enumerate() {
                    if index > 0 {
                        self.write(", ");
                    }
                    if let Some(name) = &parameter.name {
                        self.write(name);
                        self.write(": ");
                    }
                    self.format_type(&parameter.ty);
                }
                self.write("): ");
                self.format_type(&ty.return_type);
                if let Some(provenance) = &ty.result_provenance {
                    self.write(" from ");
                    for (index, origin) in provenance.origins.iter().enumerate() {
                        if index > 0 {
                            self.write(" | ");
                        }
                        self.write(origin.kind.source_label());
                    }
                }
            }
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
