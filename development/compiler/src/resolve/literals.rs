use super::{
    LiteralCaptureSignature, LiteralResolution, LiteralSignature, ParameterSignature, Resolver,
    SymbolKind, TypeSymbolKind,
};
use crate::ast::{AstFile, LiteralDecl, LiteralShape, TypeExpr};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::source::ByteSpan;

impl Resolver<'_> {
    pub(super) fn collect_literal_definitions(&mut self, ast: &AstFile) {
        for item in &ast.items {
            match item {
                crate::ast::Item::Construct(construct) => {
                    for (_, literal) in construct.literals() {
                        self.collect_literal_definition(ast, literal);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_literal_definition(&mut self, ast: &AstFile, literal: &LiteralDecl) {
        let Some(target_name) = literal_target_name(&literal.target) else {
            self.output
                .diagnostics
                .push(invalid_literal_target_diagnostic(
                    self.sources,
                    literal.target.span(),
                    "literal target must be a nominal type reference",
                ));
            return;
        };
        let Some(symbol_id) = self.output.symbols.id_by_name(target_name) else {
            self.output
                .diagnostics
                .push(invalid_literal_target_diagnostic(
                    self.sources,
                    literal.target.span(),
                    &format!("literal target type `{target_name}` is not visible"),
                ));
            return;
        };

        let validation = {
            let Some(symbol) = self.output.symbols.get(symbol_id) else {
                return;
            };
            let SymbolKind::Type(target) = &symbol.kind else {
                self.output
                    .diagnostics
                    .push(invalid_literal_target_diagnostic(
                        self.sources,
                        literal.target.span(),
                        &format!("literal target `{target_name}` is not a type"),
                    ));
                return;
            };
            if symbol.declaration_span.source != ast.span.source {
                Err("literal definitions must be declared in the target type's module".to_string())
            } else if !matches!(target.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum) {
                Err("literal target must be a nominal struct or enum".to_string())
            } else {
                validate_literal_target_parameters(literal, &target.generic_parameters)
            }
        };
        if let Err(message) = validation {
            self.output
                .diagnostics
                .push(invalid_literal_target_diagnostic(
                    self.sources,
                    literal.target.span(),
                    &message,
                ));
            return;
        }

        let Some(symbol) = self.output.symbols.get_mut(symbol_id) else {
            return;
        };
        let SymbolKind::Type(target) = &mut symbol.kind else {
            return;
        };
        if let Some(first) = target
            .literals
            .iter()
            .find(|definition| definition.shape == literal.shape)
        {
            self.output
                .diagnostics
                .push(duplicate_literal_definition_diagnostic(
                    self.sources,
                    target_name,
                    literal.shape,
                    first.shape_span,
                    literal.shape_span,
                ));
            return;
        }

        target.literals.push(literal_signature(literal));
    }

    pub(super) fn resolve_typed_literal(
        &mut self,
        target: &TypeExpr,
        shape: LiteralShape,
        expression_span: ByteSpan,
    ) {
        let Some(target_name) = literal_target_name(target) else {
            self.output
                .diagnostics
                .push(missing_literal_definition_diagnostic(
                    self.sources,
                    "<invalid target>",
                    shape,
                    target.span(),
                ));
            return;
        };
        let Some(symbol_id) = self.output.symbols.id_by_name(target_name) else {
            self.output
                .diagnostics
                .push(missing_literal_definition_diagnostic(
                    self.sources,
                    target_name,
                    shape,
                    target.span(),
                ));
            return;
        };
        let Some(symbol) = self.output.symbols.get(symbol_id) else {
            return;
        };
        let SymbolKind::Type(target_symbol) = &symbol.kind else {
            self.output
                .diagnostics
                .push(missing_literal_definition_diagnostic(
                    self.sources,
                    target_name,
                    shape,
                    target.span(),
                ));
            return;
        };
        let Some(definition) = target_symbol
            .literals
            .iter()
            .find(|definition| definition.shape == shape && definition.is_accessible)
        else {
            self.output
                .diagnostics
                .push(missing_literal_definition_diagnostic(
                    self.sources,
                    target_name,
                    shape,
                    target.span(),
                ));
            return;
        };
        self.output.typed_literal_targets.insert(
            expression_span,
            LiteralResolution {
                type_symbol: symbol_id,
                literal_declaration_span: definition.declaration_span,
            },
        );
    }
}

pub(super) fn attach_literal_definitions_to_symbol(
    symbol: &mut super::TypeSymbol,
    ast: &AstFile,
    target_name: &str,
) {
    for item in &ast.items {
        match item {
            crate::ast::Item::Construct(construct) => {
                symbol
                    .literals
                    .extend(construct.literals().filter_map(|(_, literal)| {
                        (literal_target_name(&literal.target) == Some(target_name))
                            .then(|| literal_signature(literal))
                    }));
            }
            _ => {}
        }
    }
}

fn literal_signature(literal: &LiteralDecl) -> LiteralSignature {
    LiteralSignature {
        shape: literal.shape,
        visibility: literal.visibility,
        is_accessible: true,
        declaration_span: literal.span,
        shape_span: literal.shape_span,
        capture: literal
            .capture
            .as_ref()
            .map(|capture| LiteralCaptureSignature {
                name: capture.name.clone(),
                name_span: capture.name_span,
                element_type: capture.element_type.clone(),
            }),
        parameters: literal
            .parameters
            .parameters
            .iter()
            .map(|parameter| ParameterSignature {
                name: parameter.name.clone(),
                name_span: parameter.name_span,
                ty: parameter.ty.clone(),
            })
            .collect(),
        return_type: literal.return_type.clone(),
        result_provenance: literal.result_provenance.clone(),
    }
}

fn literal_target_name(target: &TypeExpr) -> Option<&str> {
    match target {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        _ => None,
    }
}

fn validate_literal_target_parameters(
    literal: &LiteralDecl,
    generic_parameters: &[String],
) -> Result<(), String> {
    let arguments: &[TypeExpr] = match &literal.target {
        TypeExpr::Reference(_) => &[],
        TypeExpr::Generic(generic) => &generic.arguments,
        _ => return Err("literal target must be a nominal type reference".to_string()),
    };
    if arguments.len() != generic_parameters.len() {
        return Err(format!(
            "literal target must bind all {} generic parameter(s) in declaration order",
            generic_parameters.len()
        ));
    }
    for (argument, parameter) in arguments.iter().zip(generic_parameters) {
        let TypeExpr::Reference(reference) = argument else {
            return Err(
                "literal target generic arguments must name the target's parameters directly"
                    .to_string(),
            );
        };
        if &reference.name != parameter {
            return Err(format!(
                "literal target generic argument `{}` must be `{parameter}`",
                reference.name
            ));
        }
    }
    Ok(())
}

fn shape_name(shape: LiteralShape) -> &'static str {
    match shape {
        LiteralShape::Sequence => "sequence",
        LiteralShape::String => "string",
    }
}

fn invalid_literal_target_diagnostic(
    sources: &crate::source::SourceMap,
    span: ByteSpan,
    message: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0420", message);
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "declare the literal beside its nominal target and use the target's declared generic parameters"
            .to_string(),
    );
    diagnostic
}

fn duplicate_literal_definition_diagnostic(
    sources: &crate::source::SourceMap,
    target_name: &str,
    shape: LiteralShape,
    first: ByteSpan,
    duplicate: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0421",
        format!(
            "type `{target_name}` already has a {} literal definition",
            shape_name(shape)
        ),
    );
    diagnostic.primary_span = sources.span_to_json(duplicate).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first definition is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some("keep exactly one definition for each target and shape".to_string());
    diagnostic
}

fn missing_literal_definition_diagnostic(
    sources: &crate::source::SourceMap,
    target_name: &str,
    shape: LiteralShape,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0422",
        format!(
            "type `{target_name}` has no visible {} literal definition",
            shape_name(shape)
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(format!(
        "declare or import a visible `literal {target_name} {}` definition",
        match shape {
            LiteralShape::Sequence => "[](...items: T)",
            LiteralShape::String => "\"\"(text: &str)",
        }
    ));
    diagnostic
}
