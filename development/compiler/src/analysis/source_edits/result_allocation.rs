//! Source edits for explicit result-allocation contracts.

use crate::analysis::FileAnalysis;
use crate::analysis::presentation::CallableDeclarationIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResultAllocationContractEditPlan {
    pub(crate) offset: usize,
    pub(crate) new_text: &'static str,
}

/// Plans an `alloc` modifier at the AST-owned callable keyword.
///
/// The diagnostic points at the callable declaration, while the insertion
/// anchor belongs to the parsed syntax. Keeping those roles separate avoids
/// reconstructing modifier order from source text in each editor frontend.
pub(crate) fn plan_result_allocation_contract(
    file: &FileAnalysis,
    diagnostic_offset: usize,
) -> Option<ResultAllocationContractEditPlan> {
    let index = CallableDeclarationIndex::new(&file.ast);
    let anchors = index.at_declaration_offset(diagnostic_offset)?;
    Some(ResultAllocationContractEditPlan {
        offset: anchors.result_allocation_insertion?,
        new_text: "alloc ",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;

    fn apply_plan(text: &str, marker: &str) -> String {
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("root file");
        let offset = text.find(marker).expect("diagnostic marker");
        let plan = plan_result_allocation_contract(file, offset).expect("allocation edit");
        let mut edited = text.to_string();
        edited.insert_str(plan.offset, plan.new_text);
        edited
    }

    #[test]
    fn inserts_after_existing_declaration_modifiers_for_all_callable_shapes() {
        let cases = [
            (
                "pub func make(): i32 { return 0 }\n",
                "make",
                "pub alloc func make(): i32 { return 0 }\n",
            ),
            (
                "#target: \"c\"\npub primitive make(): i32\n",
                "make",
                "#target: \"c\"\npub alloc primitive make(): i32\n",
            ),
            (
                "impl i32 {\n    pub method &self.make(): i32 { return 0 }\n}\n",
                "make",
                "impl i32 {\n    pub alloc method &self.make(): i32 { return 0 }\n}\n",
            ),
            (
                "construct i32 {\n    pub default literal \"\"(text: &str): Self { return 0 }\n}\n",
                "literal",
                "construct i32 {\n    pub default alloc literal \"\"(text: &str): Self { return 0 }\n}\n",
            ),
        ];

        for (source, marker, expected) in cases {
            assert_eq!(apply_plan(source, marker), expected);
        }
    }

    #[test]
    fn planned_edit_clears_a_missing_contract_diagnostic() {
        let text = r#"#target: "c"
alloc primitive allocate(): &u8

func make(): &u8 {
    return allocate()
}
"#;
        let (_sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("root file");
        let diagnostics = analysis.diagnostics();
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "E0462")
            .expect("missing contract diagnostic");
        let offset = diagnostic
            .primary_span
            .as_ref()
            .expect("primary span")
            .start_byte;
        let plan = plan_result_allocation_contract(file, offset).expect("allocation edit");
        let mut edited = text.to_string();
        edited.insert_str(plan.offset, plan.new_text);

        let (_sources, analysis) = analyze_text(&edited);
        assert!(
            analysis
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.code != "E0462"),
            "{edited}"
        );
    }
}
