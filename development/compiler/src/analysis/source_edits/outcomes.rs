use crate::analysis::FileAnalysis;
use crate::ast::TypeExpr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutcomeContractKind {
    Fallible,
    Optional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutcomeContractEditPlan {
    pub(crate) offset: usize,
    pub(crate) new_text: &'static str,
}

pub(crate) fn plan_outcome_contract(
    file: &FileAnalysis,
    diagnostic_offset: usize,
    kind: OutcomeContractKind,
) -> Option<OutcomeContractEditPlan> {
    let body = file
        .resolved
        .semantic_db
        .body_containing(file.ast.span.source, diagnostic_offset)?;
    let owner = file
        .resolved
        .callable_bodies
        .declaration_id(body.owner)
        .unwrap_or(body.owner);
    let return_type = match file.syntax.callable(owner)? {
        crate::analysis::syntax_index::CallableSyntax::Function(function) => &function.return_type,
        crate::analysis::syntax_index::CallableSyntax::Method { method, .. }
        | crate::analysis::syntax_index::CallableSyntax::InterfaceMethod(method) => {
            &method.return_type
        }
        crate::analysis::syntax_index::CallableSyntax::Literal(literal) => &literal.return_type,
        crate::analysis::syntax_index::CallableSyntax::Primitive(_) => return None,
    };
    if matches!(
        (kind, return_type),
        (OutcomeContractKind::Fallible, TypeExpr::Fallible(_))
            | (OutcomeContractKind::Optional, TypeExpr::Optional(_))
    ) {
        return None;
    }
    Some(OutcomeContractEditPlan {
        offset: return_type.span().end,
        new_text: match kind {
            OutcomeContractKind::Fallible => "!",
            OutcomeContractKind::Optional => "?",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;

    #[test]
    fn fallible_contract_edit_fixes_local_propagation_contract() {
        let text = r#"func run(): i32 {
    return answer()?
}
func answer(): i32! { return 1 }
"#;
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().unwrap();
        let offset = text.find('?').unwrap();
        let edit = plan_outcome_contract(file, offset, OutcomeContractKind::Fallible).unwrap();
        let mut edited = text.to_string();
        edited.insert_str(edit.offset, edit.new_text);
        let (_sources, analysis) = analyze_text(&edited);
        assert!(
            analysis
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.code != "E0331"),
            "{edited}"
        );
    }
}
