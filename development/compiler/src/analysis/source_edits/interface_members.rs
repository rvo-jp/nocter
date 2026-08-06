use crate::analysis::FileAnalysis;
use crate::analysis::presentation::method_presentation;
use crate::ast::{ImplMember, Item, TypeExpr};
use crate::resolve::TypeSymbolKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterfaceMembersEditPlan {
    pub(crate) offset: usize,
    pub(crate) new_text: String,
    pub(crate) method_names: Vec<String>,
}

pub(crate) fn plan_missing_interface_members(
    file: &FileAnalysis,
    diagnostic_offset: usize,
) -> Option<InterfaceMembersEditPlan> {
    let impl_ = file.ast.items.iter().find_map(|item| {
        let Item::Impl(impl_) = item else {
            return None;
        };
        (impl_.target_ty.span().start <= diagnostic_offset
            && diagnostic_offset <= impl_.target_ty.span().end)
            .then_some(impl_)
    })?;
    let interface_name = type_reference_name(impl_.interface_ty.as_ref()?)?;
    let interface = file
        .resolved
        .type_symbol_by_reference_name(interface_name)?;
    if interface.kind != TypeSymbolKind::Interface {
        return None;
    }
    let existing = impl_
        .members
        .iter()
        .filter_map(|member| match member {
            ImplMember::Method(method) => Some(method.name.as_str()),
            ImplMember::Drop(_) => None,
        })
        .collect::<std::collections::HashSet<_>>();
    let missing = interface
        .methods
        .iter()
        .filter(|method| !method.has_default_body && !existing.contains(method.name.as_str()))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return None;
    }

    let mut new_text = String::new();
    for method in &missing {
        let signature = method_presentation(interface, method, &file.resolved)
            .render()
            .replacen("Self.", "self.", 1);
        new_text.push_str("\n    ");
        new_text.push_str(&signature);
        new_text.push_str(" {\n        loop {}\n    }\n");
    }
    Some(InterfaceMembersEditPlan {
        offset: impl_.span.end.saturating_sub(1),
        new_text,
        method_names: missing.iter().map(|method| method.name.clone()).collect(),
    })
}

fn type_reference_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        TypeExpr::Callable(_)
        | TypeExpr::Closure(_)
        | TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;

    #[test]
    fn generated_required_member_is_valid_nocter_source() {
        let text = r#"interface Printable {
    pub method &self.print(): i32
}
struct User { id: i32 }
impl Printable for User {}
"#;
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().unwrap();
        let offset = text.find("for User").unwrap() + 4;
        let edit = plan_missing_interface_members(file, offset).unwrap();
        let mut edited = text.to_string();
        edited.insert_str(edit.offset, &edit.new_text);
        let (_sources, analysis) = analyze_text(&edited);
        assert!(
            analysis
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.code != "E0425"),
            "{}",
            edited
        );
    }
}
