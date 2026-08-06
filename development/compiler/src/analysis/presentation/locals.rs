use crate::ast::TypeExpr;
use crate::resolve::{LocalSymbol, LocalSymbolKind, ResolveOutput};
use crate::typecheck::type_expr_presentation_label;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalPresentation {
    prefix: String,
    name: String,
    ty: Option<String>,
}

impl LocalPresentation {
    pub(crate) fn render(&self) -> String {
        match &self.ty {
            Some(ty) => format!("{}{}: {ty}", self.prefix, self.name),
            None => format!("{}{}", self.prefix, self.name),
        }
    }
}

pub(crate) fn local_presentation(
    symbol: &LocalSymbol,
    ty: Option<&TypeExpr>,
    resolved: &ResolveOutput,
) -> LocalPresentation {
    let prefix = match symbol.kind {
        LocalSymbolKind::Parameter => "parameter ".to_string(),
        LocalSymbolKind::Binding(crate::ast::BindingKind::Let)
        | LocalSymbolKind::ForRange
        | LocalSymbolKind::CollectionFor
        | LocalSymbolKind::LiteralPackFor => "let ".to_string(),
        LocalSymbolKind::Binding(crate::ast::BindingKind::Var) => "var ".to_string(),
        LocalSymbolKind::Region => "region ".to_string(),
        LocalSymbolKind::LiteralCapture => "literal pack ".to_string(),
        LocalSymbolKind::ClosureCapture(mode) => format!("capture {}", mode.source_prefix()),
        LocalSymbolKind::PatternPayload => "payload ".to_string(),
        LocalSymbolKind::CatchError => "catch ".to_string(),
    };
    LocalPresentation {
        prefix,
        name: symbol.name.clone(),
        ty: ty.map(|ty| type_expr_presentation_label(ty, resolved)),
    }
}
