use crate::analysis::FileAnalysis;
use crate::ast::{ConstructMemberDecl, Item, TypeExpr};

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
    let return_type =
        file.ast.items.iter().find_map(|item| match item {
            Item::Function(function)
                if function
                    .body
                    .as_ref()
                    .is_some_and(|body| contains(body.span, diagnostic_offset)) =>
            {
                Some(&function.return_type)
            }
            Item::Interface(interface) => interface.methods.iter().find_map(|method| {
                method
                    .body
                    .as_ref()
                    .filter(|body| contains(body.span, diagnostic_offset))
                    .map(|_| &method.return_type)
            }),
            Item::Instance(_) | Item::Conformance(_) => item
                .method_owner()
                .expect("matched method owner")
                .methods()
                .find_map(|method| {
                    method
                        .body
                        .as_ref()
                        .filter(|body| contains(body.span, diagnostic_offset))
                        .map(|_| &method.return_type)
                }),
            Item::Construct(construct) => construct.members.iter().find_map(|member| match &member
                .declaration
            {
                ConstructMemberDecl::Function(function)
                    if function
                        .body
                        .as_ref()
                        .is_some_and(|body| contains(body.span, diagnostic_offset)) =>
                {
                    Some(&function.return_type)
                }
                ConstructMemberDecl::Literal(literal)
                    if literal
                        .body
                        .as_ref()
                        .is_some_and(|body| contains(body.span, diagnostic_offset)) =>
                {
                    Some(&literal.return_type)
                }
                ConstructMemberDecl::Function(_) | ConstructMemberDecl::Literal(_) => None,
            }),
            Item::Coerce(coerce) => coerce
                .entries
                .iter()
                .find(|entry| {
                    entry
                        .body
                        .as_ref()
                        .is_some_and(|body| contains(body.span, diagnostic_offset))
                })
                .map(|entry| &entry.target),
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Function(_)
            | Item::Test(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_) => None,
        })?;
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

fn contains(span: crate::source::ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
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
