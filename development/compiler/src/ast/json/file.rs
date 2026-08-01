use super::*;

impl AstFile {
    pub fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let documentation = super::documentation::collect_ast_documentation(self, sources);
        let mut node = JsonAstNode::new(
            "source_file",
            json_span(sources, self.span),
            self.items
                .iter()
                .map(|item| item.to_json(sources))
                .collect(),
        )
        .with_documentation(documentation.file());
        node.apply_documentation(&documentation);
        node
    }
}
