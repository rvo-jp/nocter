//! Compiler-owned editor facts for typed literal declarations and expressions.

use super::literal_specializations::{
    LiteralSpecialization, literal_specialization_for_expression_span,
};
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{Expr, LiteralDecl, LiteralShape};
use crate::source::ByteSpan;
use crate::typecheck::type_expr_presentation_label;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiteralCursorRegion {
    Hover,
    Arguments,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiteralParameterInfo {
    pub(crate) label: String,
    pub(crate) ty: crate::ast::TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiteralEditorInfo {
    pub(crate) expression_span: ByteSpan,
    pub(crate) focus_span: ByteSpan,
    pub(crate) declaration_span: ByteSpan,
    pub(crate) declaration_shape_span: ByteSpan,
    pub(crate) label: String,
    pub(crate) parameters: Vec<LiteralParameterInfo>,
    pub(crate) result_type: crate::ast::TypeExpr,
    pub(crate) is_specialized: bool,
}

pub(crate) fn literal_editor_info_at_offset(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
    region: LiteralCursorRegion,
) -> Option<LiteralEditorInfo> {
    let site = literal_site_at_offset(file, offset, region)?;
    let specialization =
        literal_specialization_for_expression_span(analysis, file, site.expression_span)?;
    let declaration = literal_declaration(analysis, specialization.def_id)?;
    Some(editor_info(file, site, declaration, specialization, offset))
}

pub(crate) fn literal_arguments_contain_offset(file: &FileAnalysis, offset: usize) -> bool {
    literal_site_at_offset(file, offset, LiteralCursorRegion::Arguments).is_some()
}

#[cfg(test)]
pub(crate) fn literal_definition_span_at_offset(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<ByteSpan> {
    literal_definition_target_at_offset(analysis, file, offset)
        .map(|target| target.declaration_span)
}

#[cfg(test)]
pub(crate) fn literal_definition_target_at_offset(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<crate::analysis::editor_targets::SourceTarget> {
    let site = literal_site_at_offset(file, offset, LiteralCursorRegion::Hover)?;
    let specialization =
        literal_specialization_for_expression_span(analysis, file, site.expression_span)?;
    Some(crate::analysis::editor_targets::SourceTarget::new(
        literal_focus_span(site, offset),
        analysis
            .semantic_db
            .definition_anchor(specialization.def_id)?,
    ))
}

pub(crate) type LiteralSite = super::syntax_index::LiteralSyntaxSite;

fn literal_site_at_offset(
    file: &FileAnalysis,
    offset: usize,
    region: LiteralCursorRegion,
) -> Option<LiteralSite> {
    file.syntax
        .literal_at(offset, region == LiteralCursorRegion::Arguments)
}

pub(crate) fn literal_site(expression: &Expr) -> Option<LiteralSite> {
    match expression {
        Expr::TypedSequenceLiteral(literal) => {
            let left = ByteSpan::new(
                literal.elements_span.source,
                literal.elements_span.start,
                (literal.elements_span.start + 1).min(literal.elements_span.end),
            );
            let right_start = literal.elements_span.end.saturating_sub(1);
            Some(LiteralSite {
                expression_span: literal.span,
                target_span: literal.target.span(),
                argument_span: literal.elements_span,
                left_delimiter_span: left,
                right_delimiter_span: ByteSpan::new(
                    literal.elements_span.source,
                    right_start,
                    literal.elements_span.end,
                ),
                shape: LiteralShape::Sequence,
            })
        }
        Expr::TypedStringLiteral(literal) => Some(LiteralSite {
            expression_span: literal.span,
            target_span: literal.target.span(),
            argument_span: literal.text.span,
            left_delimiter_span: literal.text.span,
            right_delimiter_span: literal.text.span,
            shape: LiteralShape::String,
        }),
        _ => None,
    }
}

pub(crate) fn literal_navigation_spans(expression: &Expr) -> Option<Vec<ByteSpan>> {
    let site = literal_site(expression)?;
    let mut spans = vec![
        site.target_span,
        site.left_delimiter_span,
        site.right_delimiter_span,
    ];
    spans.sort_by_key(|span| (span.start, span.end));
    spans.dedup();
    Some(spans)
}

fn literal_focus_span(site: LiteralSite, offset: usize) -> ByteSpan {
    if span_contains(site.target_span, offset) {
        site.target_span
    } else if span_contains(site.left_delimiter_span, offset) {
        site.left_delimiter_span
    } else {
        site.right_delimiter_span
    }
}

fn literal_declaration(
    analysis: &CompileUnitAnalysis,
    definition: crate::semantic::DefId,
) -> Option<&LiteralDecl> {
    analysis.files.iter().find_map(|file| {
        let super::syntax_index::CallableSyntax::Literal(literal) =
            file.syntax.callable(definition)?
        else {
            return None;
        };
        Some(literal)
    })
}

fn editor_info(
    file: &FileAnalysis,
    site: LiteralSite,
    declaration: &LiteralDecl,
    specialization: LiteralSpecialization,
    offset: usize,
) -> LiteralEditorInfo {
    let result = type_expr_presentation_label(&specialization.result_type, &file.resolved);
    let parameters = match site.shape {
        LiteralShape::Sequence => declaration
            .capture
            .as_ref()
            .zip(specialization.element_type.as_ref())
            .map(|(capture, element_type)| {
                vec![LiteralParameterInfo {
                    label: format!(
                        "...{}: {}",
                        capture.name,
                        type_expr_presentation_label(element_type, &file.resolved)
                    ),
                    ty: element_type.clone(),
                }]
            })
            .unwrap_or_default(),
        LiteralShape::String => declaration
            .parameters
            .parameters
            .iter()
            .zip(&specialization.argument_types)
            .map(|(parameter, ty)| LiteralParameterInfo {
                label: format!(
                    "{}: {}",
                    parameter.name,
                    type_expr_presentation_label(ty, &file.resolved)
                ),
                ty: ty.clone(),
            })
            .collect(),
    };
    let shape = match site.shape {
        LiteralShape::Sequence => "[]",
        LiteralShape::String => "\"\"",
    };
    let label = crate::analysis::presentation::LiteralPresentation::new(
        result.clone(),
        shape,
        parameters
            .iter()
            .map(|parameter| parameter.label.clone())
            .collect(),
        result,
        crate::analysis::presentation::result_origin_labels(declaration.result_provenance.as_ref()),
    )
    .render();
    LiteralEditorInfo {
        expression_span: site.expression_span,
        focus_span: literal_focus_span(site, offset),
        declaration_span: specialization.declaration_span,
        declaration_shape_span: declaration.shape_span,
        label,
        parameters,
        result_type: specialization.result_type,
        is_specialized: !specialization.substitutions.is_empty(),
    }
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;

    const SOURCE: &str = r#"struct Bucket<T> { length: usize }

construct Bucket<T> {
    /// Constructs a bucket.
    pub default literal [](...items: T): Self {
        return Bucket<T> { length: items.len() }
    }
}

func main(): i32 {
    let first = Bucket [1, 2]
    let second = Bucket [3, 4]
    return 0
}
"#;

    #[test]
    fn reports_specialized_sequence_signature_for_every_expression() {
        let (_sources, analysis) = analyze_text(SOURCE);
        let file = analysis.root_file().expect("expected root file");

        for offset in [
            SOURCE.find("Bucket [1").unwrap(),
            SOURCE.rfind("Bucket [3").unwrap(),
        ] {
            let info =
                literal_editor_info_at_offset(&analysis, file, offset, LiteralCursorRegion::Hover)
                    .expect("expected literal info");
            assert_eq!(
                info.label,
                "literal Bucket<i32> [](...items: i32): Bucket<i32>"
            );
            assert_eq!(info.parameters[0].label, "...items: i32");
            assert!(info.is_specialized);
        }
    }

    #[test]
    fn maps_expression_to_literal_shape_declaration() {
        let (_sources, analysis) = analyze_text(SOURCE);
        let file = analysis.root_file().expect("expected root file");
        let offset = SOURCE.find("[1, 2]").unwrap();
        let span = literal_definition_span_at_offset(&analysis, file, offset)
            .expect("expected literal declaration");

        assert_eq!(&SOURCE[span.start..span.end], "[]");
        assert_eq!(span.start, SOURCE.find("[](...items").unwrap());
    }

    #[test]
    fn preserves_generic_element_facts_for_editor_queries() {
        let text = r#"struct Bucket<T> { length: usize }

construct Bucket<T> {
    pub default literal [](...items: T): Self {
        return Bucket<T> { length: items.len() }
    }
}

func build<T>(value: T): Bucket<T> {
    return Bucket [move value]
}
"#;
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("Bucket [").unwrap();

        let info =
            literal_editor_info_at_offset(&analysis, file, offset, LiteralCursorRegion::Hover)
                .expect("expected generic literal info");

        assert_eq!(info.label, "literal Bucket<T> [](...items: T): Bucket<T>");
        assert!(info.is_specialized);
    }
}
