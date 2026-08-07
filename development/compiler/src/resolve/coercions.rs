//! Coherence and type-owned indexing for borrowed-view coercions.

use super::{CoercionSignature, Resolver, SymbolKind, TypeSymbol, TypeSymbolKind};
use crate::ast::{
    CoerceDecl, CoercionEntry, MethodReceiverMode, ResultProvenanceOriginKind, TypeExpr,
};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::source::ByteSpan;

impl Resolver<'_> {
    pub(super) fn collect_coercion_surfaces(&mut self, ast: &crate::ast::AstFile) {
        for item in &ast.items {
            let crate::ast::Item::Coerce(coerce) = item else {
                continue;
            };
            self.collect_coercion_surface(ast, coerce);
        }
    }

    fn collect_coercion_surface(&mut self, ast: &crate::ast::AstFile, coerce: &CoerceDecl) {
        let Some(target_name) = nominal_name(&coerce.target) else {
            return;
        };
        let Some(symbol_id) = self.output.symbols.id_by_name(target_name) else {
            self.push_coercion_error(
                format!("coerce source type `{target_name}` is not visible"),
                coerce.target.span(),
                None,
            );
            return;
        };

        let validation = self.output.symbols.get(symbol_id).and_then(|symbol| {
            let SymbolKind::Type(target) = &symbol.kind else {
                return None;
            };
            Some(validate_source(ast, coerce, symbol, target))
        });
        match validation {
            Some(Ok(())) => {}
            Some(Err((message, note))) => {
                self.push_coercion_error(message, coerce.target.span(), note);
                return;
            }
            None => {
                self.push_coercion_error(
                    format!("coerce source `{target_name}` is not a type"),
                    coerce.target.span(),
                    None,
                );
                return;
            }
        }

        let mut accepted = Vec::new();
        for entry in &coerce.entries {
            match validate_entry(entry) {
                Ok(()) => accepted.push(coercion_signature(entry)),
                Err((message, span)) => self.push_coercion_error(message, span, None),
            }
        }

        let Some(symbol) = self.output.symbols.get_mut(symbol_id) else {
            return;
        };
        let SymbolKind::Type(target) = &mut symbol.kind else {
            return;
        };
        for signature in accepted {
            let key = coercion_key(&signature);
            if let Some(first) = target
                .coercions
                .iter()
                .find(|existing| coercion_key(existing) == key)
            {
                self.output.diagnostics.push(coercion_diagnostic(
                    self.sources,
                    format!(
                        "type `{target_name}` already defines coercion `{}`",
                        display_key(&signature)
                    ),
                    signature.focus_span,
                    Some(("first coercion is declared here", first.focus_span)),
                ));
                continue;
            }
            target.coercions.push(signature);
        }
    }

    fn push_coercion_error(
        &mut self,
        message: impl Into<String>,
        span: ByteSpan,
        note: Option<(&str, ByteSpan)>,
    ) {
        self.output
            .diagnostics
            .push(coercion_diagnostic(self.sources, message, span, note));
    }
}

type ValidationError = (String, Option<(&'static str, ByteSpan)>);

fn validate_source(
    ast: &crate::ast::AstFile,
    coerce: &CoerceDecl,
    symbol: &super::Symbol,
    target: &TypeSymbol,
) -> Result<(), ValidationError> {
    if symbol.declaration_span.source != ast.span.source {
        return Err((
            "coerce declarations must be in the source type's module".to_string(),
            Some(("source type is declared here", symbol.declaration_span)),
        ));
    }
    if !matches!(target.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum) {
        return Err((
            "coerce source must be a nominal struct or enum".to_string(),
            Some(("source type is declared here", symbol.declaration_span)),
        ));
    }
    let arguments = match &coerce.target {
        TypeExpr::Reference(_) => &[][..],
        TypeExpr::Generic(generic) => generic.arguments.as_slice(),
        _ => unreachable!("parser accepts only nominal coerce sources"),
    };
    if arguments.len() != target.generic_parameters.len() {
        return Err((
            format!(
                "coerce source must bind all {} generic parameter(s) in declaration order",
                target.generic_parameters.len()
            ),
            Some(("source type is declared here", symbol.declaration_span)),
        ));
    }
    for (argument, expected) in arguments.iter().zip(&target.generic_parameters) {
        let TypeExpr::Reference(reference) = argument else {
            return Err((
                "coerce source arguments must be generic parameter names".to_string(),
                None,
            ));
        };
        if reference.name != *expected {
            return Err((
                format!(
                    "coerce source generic argument `{}` must be `{expected}`",
                    reference.name
                ),
                None,
            ));
        }
    }
    Ok(())
}

fn validate_entry(entry: &CoercionEntry) -> Result<(), (String, ByteSpan)> {
    if !is_borrowed_target(&entry.target) {
        return Err((
            "coercion target must be a borrowed type or view".to_string(),
            entry.target.span(),
        ));
    }
    if entry.receiver.mode == MethodReceiverMode::ReadonlyBorrow
        && target_is_readwrite(&entry.target)
    {
        return Err((
            "readonly coercion receiver cannot produce a readwrite target".to_string(),
            entry.target.span(),
        ));
    }
    if let Some(provenance) = &entry.result_provenance
        && (provenance.origins.len() != 1
            || provenance.origins[0].kind != ResultProvenanceOriginKind::Receiver)
    {
        return Err((
            "borrow coercion result provenance must be exactly `from self`".to_string(),
            provenance.span,
        ));
    }
    Ok(())
}

fn is_borrowed_target(target: &TypeExpr) -> bool {
    matches!(target, TypeExpr::Borrow(_) | TypeExpr::View(_))
}

fn target_is_readwrite(target: &TypeExpr) -> bool {
    match target {
        TypeExpr::Borrow(borrow) => borrow.is_readwrite,
        TypeExpr::View(view) => view.is_readwrite,
        _ => false,
    }
}

fn nominal_name(target: &TypeExpr) -> Option<&str> {
    match target {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        _ => None,
    }
}

fn coercion_signature(entry: &CoercionEntry) -> CoercionSignature {
    CoercionSignature {
        declaration_span: entry.span,
        focus_span: entry.as_span,
        visibility: entry.visibility,
        is_accessible: true,
        receiver: entry.receiver.clone(),
        target: entry.target.clone(),
        result_provenance: entry.result_provenance.clone(),
    }
}

fn coercion_key(signature: &CoercionSignature) -> (MethodReceiverMode, String) {
    (
        signature.receiver.mode,
        crate::ast::canonical_type_expr(&signature.target),
    )
}

fn display_key(signature: &CoercionSignature) -> String {
    format!(
        "{}self as {}",
        signature.receiver.mode.source_prefix(),
        crate::ast::canonical_type_expr(&signature.target)
    )
}

fn coercion_diagnostic(
    sources: &crate::source::SourceMap,
    message: impl Into<String>,
    primary: ByteSpan,
    note: Option<(&str, ByteSpan)>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0465", message);
    diagnostic.primary_span = sources.span_to_json(primary).ok().map(Box::new);
    if let Some((message, span)) = note
        && let Ok(span) = sources.span_to_json(span)
    {
        diagnostic.notes.push(DiagnosticNote {
            message: message.to_string(),
            span: Some(span),
        });
    }
    diagnostic
}

pub(super) fn attach_coercions_to_symbol(
    symbol: &mut TypeSymbol,
    ast: &crate::ast::AstFile,
    expected_source_name: &str,
) {
    for item in &ast.items {
        let crate::ast::Item::Coerce(coerce) = item else {
            continue;
        };
        if nominal_name(&coerce.target) != Some(expected_source_name) {
            continue;
        }
        for entry in &coerce.entries {
            if validate_entry(entry).is_ok() {
                symbol.coercions.push(coercion_signature(entry));
            }
        }
    }
}
