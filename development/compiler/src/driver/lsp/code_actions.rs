use super::auto_import::{AutoImportContext, auto_import_candidates, import_edit_value};
use super::documents::OpenDocument;
use super::protocol::{lsp_position_to_byte_offset, range_for_byte_span};
use crate::analysis::FileAnalysis;
use crate::analysis::source_edits::{
    OutcomeContractKind, plan_missing_interface_members, plan_outcome_contract,
    plan_result_allocation_contract,
};
use crate::source::{ByteSpan, SourceId};
use serde_json::{Value, json};

pub(super) fn code_actions(
    context: &AutoImportContext<'_>,
    file: &FileAnalysis,
    params: Option<&Value>,
) -> Vec<Value> {
    let Some((requested_start, requested_end)) = requested_byte_range(context.document, params)
    else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    for diagnostic in &file.diagnostics {
        let Some(span) = diagnostic.primary_span.as_deref() else {
            continue;
        };
        if !diagnostic_belongs_to_document(context.document, span)
            || span.end_byte < requested_start
            || requested_end < span.start_byte
        {
            continue;
        }
        match diagnostic.code.as_str() {
            "E0416" => add_import_actions(&mut actions, context, span.start_byte, span.end_byte),
            "E0425" => {
                if let Some(plan) = plan_missing_interface_members(file, span.start_byte) {
                    let title = if plan.method_names.len() == 1 {
                        format!("Implement required method `{}`", plan.method_names[0])
                    } else {
                        format!("Implement {} required methods", plan.method_names.len())
                    };
                    actions.push(quick_fix(
                        context.document,
                        title,
                        plan.offset,
                        plan.offset,
                        plan.new_text,
                        true,
                    ));
                }
            }
            "E0331" => add_outcome_action(
                &mut actions,
                context.document,
                file,
                span.start_byte,
                OutcomeContractKind::Fallible,
            ),
            "E0335" => add_outcome_action(
                &mut actions,
                context.document,
                file,
                span.start_byte,
                OutcomeContractKind::Optional,
            ),
            "E0462" => {
                if let Some(plan) = plan_result_allocation_contract(file, span.start_byte) {
                    actions.push(quick_fix(
                        context.document,
                        "Mark the callable result as allocated".to_string(),
                        plan.offset,
                        plan.offset,
                        plan.new_text.to_string(),
                        true,
                    ));
                }
            }
            _ => {}
        }
    }
    actions
}

fn add_import_actions(
    actions: &mut Vec<Value>,
    context: &AutoImportContext<'_>,
    start: usize,
    end: usize,
) {
    let Some(name) = context.document.text.get(start..end) else {
        return;
    };
    let candidates = auto_import_candidates(context, Some(name))
        .into_iter()
        .filter(|candidate| candidate.name == name)
        .collect::<Vec<_>>();
    let preferred = candidates.len() == 1;
    for candidate in candidates {
        let Some(edit) = import_edit_value(context, &candidate) else {
            continue;
        };
        actions.push(json!({
            "title": format!("Import `{}` from `{}`", candidate.name, candidate.module_path),
            "kind": "quickfix",
            "isPreferred": preferred,
            "edit": {
                "documentChanges": [{
                    "textDocument": {
                        "uri": context.document.uri,
                        "version": context.document.version
                    },
                    "edits": [edit]
                }]
            }
        }));
    }
}

fn add_outcome_action(
    actions: &mut Vec<Value>,
    document: &OpenDocument,
    file: &FileAnalysis,
    offset: usize,
    kind: OutcomeContractKind,
) {
    let Some(plan) = plan_outcome_contract(file, offset, kind) else {
        return;
    };
    let title = match kind {
        OutcomeContractKind::Fallible => "Make the enclosing callable fallible",
        OutcomeContractKind::Optional => "Make the enclosing callable optional",
    };
    actions.push(quick_fix(
        document,
        title.to_string(),
        plan.offset,
        plan.offset,
        plan.new_text.to_string(),
        true,
    ));
}

fn quick_fix(
    document: &OpenDocument,
    title: String,
    start: usize,
    end: usize,
    new_text: String,
    preferred: bool,
) -> Value {
    json!({
        "title": title,
        "kind": "quickfix",
        "isPreferred": preferred,
        "edit": {
            "documentChanges": [{
                "textDocument": {
                    "uri": document.uri,
                    "version": document.version
                },
                "edits": [{
                    "range": range_for_byte_span(
                        &document.text,
                        ByteSpan::new(SourceId::new(0), start, end),
                    ),
                    "newText": new_text
                }]
            }]
        }
    })
}

fn requested_byte_range(document: &OpenDocument, params: Option<&Value>) -> Option<(usize, usize)> {
    let range = params?.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;
    Some((
        lsp_position_to_byte_offset(
            &document.text,
            start.get("line")?.as_u64()? as usize,
            start.get("character")?.as_u64()? as usize,
        ),
        lsp_position_to_byte_offset(
            &document.text,
            end.get("line")?.as_u64()? as usize,
            end.get("character")?.as_u64()? as usize,
        ),
    ))
}

fn diagnostic_belongs_to_document(document: &OpenDocument, span: &crate::source::JsonSpan) -> bool {
    if let (Some(document_path), Some(span_path)) = (&document.absolute_path, &span.absolute_path) {
        return document_path == std::path::Path::new(span_path);
    }
    span.file == document.display_path || span.file == document.uri
}
