use super::Formatter;
use crate::ast::{TypeExpr, canonical_type_expr};

impl Formatter {
    pub(super) fn format_type(&mut self, ty: &TypeExpr) {
        self.write(&canonical_type_expr(ty));
    }
}
