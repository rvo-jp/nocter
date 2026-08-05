//! Validation and type-owned indexing for direct construction APIs.

use super::{
    ConstructionEntry, ConstructionEntryKind, ConstructionSurface, Resolver, SymbolKind,
    TypeSymbol, TypeSymbolKind,
};
use crate::ast::{ConstructDecl, ConstructMemberDecl, TypeExpr, Visibility};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::source::ByteSpan;

impl Resolver<'_> {
    pub(super) fn collect_construction_surfaces(&mut self, ast: &crate::ast::AstFile) {
        self.reject_detached_construction_functions(ast);
        for item in &ast.items {
            let crate::ast::Item::Construct(construct) = item else {
                continue;
            };
            self.collect_construction_surface(ast, construct);
        }
    }

    fn reject_detached_construction_functions(&mut self, ast: &crate::ast::AstFile) {
        for item in &ast.items {
            let crate::ast::Item::Function(function) = item else {
                continue;
            };
            let Some(owner) = &function.owner else {
                continue;
            };
            let Some(symbol) = self
                .output
                .symbols
                .id_by_name(&owner.name)
                .and_then(|id| self.output.symbols.get(id))
            else {
                continue;
            };
            let SymbolKind::Type(target) = &symbol.kind else {
                continue;
            };
            if !matches!(target.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum)
                || !result_constructs_owner(&function.return_type, &owner.name)
            {
                continue;
            }
            self.output.diagnostics.push(construction_diagnostic(
                self.sources,
                format!(
                    "construction function `{}` must be declared inside `construct {} {{ ... }}`",
                    function.name, owner.name
                ),
                function.member_name_span,
                None,
            ));
        }
    }

    fn collect_construction_surface(
        &mut self,
        ast: &crate::ast::AstFile,
        construct: &ConstructDecl,
    ) {
        let Some(target_name) = target_name(&construct.target) else {
            self.push_error(
                "construct target must be a nominal type reference",
                construct.target.span(),
                None,
            );
            return;
        };
        let Some(symbol_id) = self.output.symbols.id_by_name(target_name) else {
            self.push_error(
                format!("construct target type `{target_name}` is not visible"),
                construct.target.span(),
                None,
            );
            return;
        };

        let validation = self
            .output
            .symbols
            .get(symbol_id)
            .and_then(|symbol| match &symbol.kind {
                SymbolKind::Type(target) => Some(validate_target(ast, construct, symbol, target)),
                _ => None,
            });
        match validation {
            Some(Ok(())) => {}
            Some(Err((message, note))) => {
                self.push_error(message, construct.target.span(), note);
                return;
            }
            None => {
                self.push_error(
                    format!("construct target `{target_name}` is not a type"),
                    construct.target.span(),
                    None,
                );
                return;
            }
        }

        let Some(symbol) = self.output.symbols.get_mut(symbol_id) else {
            return;
        };
        let SymbolKind::Type(target) = &mut symbol.kind else {
            return;
        };
        if let Some(first) = target.construction.declaration_span {
            self.output.diagnostics.push(construction_diagnostic(
                self.sources,
                format!("type `{target_name}` already has a construct declaration"),
                construct.target.span(),
                Some(("first construct declaration is here", first)),
            ));
            return;
        }

        target.construction.declaration_span = Some(construct.span);
        for member in &construct.members {
            match &member.declaration {
                ConstructMemberDecl::Function(function) => {
                    if !success_payload_is_self(&function.return_type) {
                        self.output.diagnostics.push(construction_diagnostic(
                            self.sources,
                            format!(
                                "construction function `{}` must produce `Self`",
                                function.member_name
                            ),
                            function.return_type.span(),
                            None,
                        ));
                    }
                }
                ConstructMemberDecl::Literal(literal) => {
                    if !success_payload_is_self(&literal.return_type) {
                        self.output.diagnostics.push(construction_diagnostic(
                            self.sources,
                            "literal construction member must produce `Self`",
                            literal.return_type.span(),
                            None,
                        ));
                    }
                }
            }
        }

        let explicit_defaults = append_construction_entries(target, construct);
        if let Some(&(entry, first_span)) = explicit_defaults.first() {
            target.construction.default_entry = Some(entry);
            hide_structural_entry(&mut target.construction);
            for &(_, duplicate_span) in explicit_defaults.iter().skip(1) {
                self.output.diagnostics.push(construction_diagnostic(
                    self.sources,
                    "a construct declaration may have only one default member",
                    duplicate_span,
                    Some(("first default member is here", first_span)),
                ));
            }
        } else if !construct.members.is_empty()
            && !target
                .fields
                .iter()
                .all(|field| field.visibility == Visibility::Public)
        {
            self.output.diagnostics.push(construction_diagnostic(
                self.sources,
                format!(
                    "construct declaration for `{target_name}` requires a default member because structural construction is not public"
                ),
                construct.target.span(),
                None,
            ));
        }
    }

    fn push_error(
        &mut self,
        message: impl Into<String>,
        span: ByteSpan,
        note: Option<(&str, ByteSpan)>,
    ) {
        self.output
            .diagnostics
            .push(construction_diagnostic(self.sources, message, span, note));
    }
}

fn validate_target(
    ast: &crate::ast::AstFile,
    construct: &ConstructDecl,
    symbol: &super::Symbol,
    target: &TypeSymbol,
) -> Result<(), (String, Option<(&'static str, ByteSpan)>)> {
    if symbol.declaration_span.source != ast.span.source {
        return Err((
            "construct declarations must be in the target type's module".to_string(),
            Some(("target type is declared here", symbol.declaration_span)),
        ));
    }
    if !matches!(target.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum) {
        return Err((
            "construct target must be a nominal struct or enum".to_string(),
            Some(("target is declared here", symbol.declaration_span)),
        ));
    }
    let arguments = match &construct.target {
        TypeExpr::Reference(_) => &[][..],
        TypeExpr::Generic(generic) => generic.arguments.as_slice(),
        _ => unreachable!("target name validation rejected non-nominal syntax"),
    };
    if arguments.len() != target.generic_parameters.len() {
        return Err((
            format!(
                "construct target must bind all {} generic parameter(s) in declaration order",
                target.generic_parameters.len()
            ),
            Some(("target type is declared here", symbol.declaration_span)),
        ));
    }
    for (argument, expected) in arguments.iter().zip(&target.generic_parameters) {
        let TypeExpr::Reference(reference) = argument else {
            return Err((
                "construct target arguments must be the target's declared generic parameters"
                    .to_string(),
                None,
            ));
        };
        if reference.name != *expected {
            return Err((
                format!(
                    "construct target generic argument `{}` must be `{expected}`",
                    reference.name
                ),
                None,
            ));
        }
    }
    Ok(())
}

fn target_name(target: &TypeExpr) -> Option<&str> {
    match target {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        _ => None,
    }
}

fn success_payload_is_self(ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Reference(reference) => reference.name == "Self",
        TypeExpr::Optional(optional) => success_payload_is_self(&optional.inner),
        TypeExpr::Fallible(fallible) => success_payload_is_self(&fallible.success),
        _ => false,
    }
}

fn result_constructs_owner(ty: &TypeExpr, owner: &str) -> bool {
    match ty {
        TypeExpr::Reference(reference) => reference.name == "Self" || reference.name == owner,
        TypeExpr::Generic(generic) => generic.name == owner,
        TypeExpr::Optional(optional) => result_constructs_owner(&optional.inner, owner),
        TypeExpr::Fallible(fallible) => result_constructs_owner(&fallible.success, owner),
        _ => false,
    }
}

fn construction_diagnostic(
    sources: &crate::source::SourceMap,
    message: impl Into<String>,
    primary: ByteSpan,
    note: Option<(&str, ByteSpan)>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0460", message);
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

pub(super) fn attach_construction_surfaces_to_symbol(
    symbol: &mut TypeSymbol,
    ast: &crate::ast::AstFile,
    expected_target_name: &str,
) {
    let Some(construct) = ast.items.iter().find_map(|item| {
        let crate::ast::Item::Construct(construct) = item else {
            return None;
        };
        (target_name(&construct.target) == Some(expected_target_name)).then_some(construct)
    }) else {
        return;
    };

    symbol.construction.declaration_span = Some(construct.span);
    let explicit_defaults = append_construction_entries(symbol, construct);
    if let Some(&(default_entry, _)) = explicit_defaults.first() {
        symbol.construction.default_entry = Some(default_entry);
        hide_structural_entry(&mut symbol.construction);
    }
}

fn append_construction_entries(
    symbol: &mut TypeSymbol,
    construct: &ConstructDecl,
) -> Vec<(usize, ByteSpan)> {
    let mut explicit_defaults = Vec::new();
    for member in &construct.members {
        let kind = match &member.declaration {
            ConstructMemberDecl::Function(function) => {
                if let Some(signature) = symbol
                    .associated_functions
                    .iter_mut()
                    .find(|signature| signature.name_span == function.member_name_span)
                {
                    for (bounds, owner_bounds) in signature
                        .signature
                        .generic_parameter_bounds
                        .iter_mut()
                        .zip(&symbol.generic_parameter_bounds)
                    {
                        *bounds = owner_bounds.clone();
                    }
                }
                ConstructionEntryKind::Function(function.member_name.clone())
            }
            ConstructMemberDecl::Literal(literal) => ConstructionEntryKind::Literal(literal.shape),
        };
        let entry_index = symbol.construction.entries.len();
        symbol.construction.entries.push(ConstructionEntry {
            kind,
            declaration_span: member.span,
            focus_span: match &member.declaration {
                ConstructMemberDecl::Function(function) => function.member_name_span,
                ConstructMemberDecl::Literal(literal) => literal.shape_span,
            },
            is_accessible: true,
        });
        if let Some(default_span) = member.default_span {
            explicit_defaults.push((entry_index, default_span));
        }
    }
    explicit_defaults
}

fn hide_structural_entry(surface: &mut ConstructionSurface) {
    if let Some(entry) = surface
        .entries
        .iter_mut()
        .find(|entry| entry.kind == ConstructionEntryKind::Structural)
    {
        entry.is_accessible = false;
    }
}
