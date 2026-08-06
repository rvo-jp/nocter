use crate::ast::{AstFile, Item};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportEditPlan {
    pub(crate) offset: usize,
    pub(crate) new_text: String,
}

pub(crate) fn plan_top_level_import(
    text: &str,
    ast: &AstFile,
    module_path: &str,
    name: &str,
) -> Option<ImportEditPlan> {
    let declaration = format!("use {module_path}.{name}");
    if ast.items.iter().any(|item| match item {
        Item::Import(import) => {
            import.path.value == format!("{module_path}.{name}")
                || (import.path.value == module_path && import.alias.name == name)
        }
        Item::FromImport(import) => {
            import.path.value == module_path
                && import
                    .names
                    .iter()
                    .any(|imported| imported.local_name() == name)
        }
        _ => false,
    }) {
        return None;
    }

    if let Some(last_import) = ast
        .items
        .iter()
        .filter(|item| matches!(item, Item::Import(_) | Item::FromImport(_)))
        .map(Item::span)
        .max_by_key(|span| span.end)
    {
        let offset = line_end_including_newline(text, last_import.end);
        return Some(ImportEditPlan {
            offset,
            new_text: format!("{declaration}\n"),
        });
    }

    let offset = ast
        .items
        .first()
        .map(|item| attached_documentation_start(text, item.span().start))
        .unwrap_or(text.len());
    let prefix = if offset > 0 && !text[..offset].ends_with('\n') {
        "\n"
    } else {
        ""
    };
    Some(ImportEditPlan {
        offset,
        new_text: format!("{prefix}{declaration}\n\n"),
    })
}

fn line_end_including_newline(text: &str, offset: usize) -> usize {
    text.get(offset..)
        .and_then(|suffix| suffix.find('\n'))
        .map(|end| offset + end + 1)
        .unwrap_or(text.len())
}

fn attached_documentation_start(text: &str, declaration_start: usize) -> usize {
    let mut start = text[..declaration_start.min(text.len())]
        .rfind('\n')
        .map(|newline| newline + 1)
        .unwrap_or(0);
    loop {
        if start == 0 {
            break;
        }
        let previous_end = start - 1;
        let previous_start = text[..previous_end]
            .rfind('\n')
            .map(|newline| newline + 1)
            .unwrap_or(0);
        if !text[previous_start..previous_end]
            .trim_start()
            .starts_with("///")
        {
            break;
        }
        start = previous_start;
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::single_file::parse_single_file_text;

    #[test]
    fn inserts_before_attached_docs_and_after_existing_imports() {
        let text = "/// Runs.\nfunc main(): i32 { return 0 }\n";
        let parsed = parse_single_file_text("app.nct", text).unwrap();
        let edit = plan_top_level_import(text, &parsed.ast, "/lib", "answer").unwrap();
        assert_eq!(edit.offset, 0);
        assert_eq!(edit.new_text, "use /lib.answer\n\n");

        let text = "use /first.one\n\nfunc main(): i32 { return 0 }\n";
        let parsed = parse_single_file_text("app.nct", text).unwrap();
        let edit = plan_top_level_import(text, &parsed.ast, "/lib", "answer").unwrap();
        assert_eq!(&text[..edit.offset], "use /first.one\n");
        assert_eq!(edit.new_text, "use /lib.answer\n");
    }
}
