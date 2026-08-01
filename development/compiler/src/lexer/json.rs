use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonToken {
    pub kind: String,
    pub lexeme: String,
    pub span: JsonSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TokensEnvelope {
    pub schema: &'static str,
    pub version: u32,
    pub ok: bool,
    pub command: &'static str,
    pub file: String,
    pub absolute_path: Option<String>,
    pub tokens: Vec<JsonToken>,
    pub diagnostics: Vec<Diagnostic>,
}

impl TokensEnvelope {
    pub fn new(
        file: impl Into<String>,
        absolute_path: Option<String>,
        tokens: Vec<JsonToken>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        let ok = diagnostics.is_empty();

        Self {
            schema: "nocter.tokens",
            version: 1,
            ok,
            command: "tokens",
            file: file.into(),
            absolute_path,
            tokens,
            diagnostics,
        }
    }
}

impl LexOutput {
    pub fn to_json_envelope(
        &self,
        sources: &SourceMap,
        source: SourceId,
    ) -> Result<TokensEnvelope, String> {
        let file = sources
            .get(source)
            .ok_or_else(|| format!("unknown source id {}", source.raw()))?;
        let mut json_tokens = Vec::with_capacity(self.tokens.len());

        for token in &self.tokens {
            let span = sources.span_to_json(token.span)?;
            let lexeme = file
                .text()
                .get(token.span.start..token.span.end)
                .unwrap_or("")
                .to_string();
            json_tokens.push(JsonToken {
                kind: token.kind.json_kind().to_string(),
                lexeme,
                span,
            });
        }

        Ok(TokensEnvelope::new(
            file.display_path().to_string(),
            file.absolute_path()
                .map(|path| path.to_string_lossy().into_owned()),
            json_tokens,
            self.diagnostics.clone(),
        ))
    }
}
