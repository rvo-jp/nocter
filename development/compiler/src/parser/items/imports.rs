use super::*;

impl Parser<'_> {
    pub(in crate::parser) fn parse_use_item(
        &mut self,
        visibility: Visibility,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Use, "`use`")?;
        let path = self.parse_module_path()?;

        if path.value == "std/prelude" {
            self.error_at(
                path.span,
                "`std/prelude` is compiler-managed and cannot be imported in source",
            );
            return Err(());
        }

        if self.match_keyword(Keyword::As).is_some() {
            if visibility != Visibility::Private {
                self.error_current("namespace aliases cannot be re-exported");
                return Err(());
            }
            let alias = self.expect_name_identifier("expected import alias after `as`")?;
            return Ok(Item::Import(ImportItem {
                span: self.span(start.span.start, alias.span.end),
                visibility,
                path,
                alias: ImportAlias {
                    span: alias.span,
                    name: alias.value,
                },
                alias_is_default: false,
            }));
        }

        if self.match_punctuation(".").is_some() {
            let (names, end) = if self.match_punctuation("{").is_some() {
                self.skip_newlines();
                let first = self.parse_imported_name("expected an imported name after `{`")?;
                let mut names = vec![first];

                loop {
                    self.skip_newlines();
                    if self.match_punctuation(",").is_none() {
                        break;
                    }
                    self.skip_newlines();
                    if self.at_punctuation("}") {
                        break;
                    }
                    let name = self.parse_imported_name("expected an imported name after `,`")?;
                    names.push(name);
                }

                self.skip_newlines();
                let close = self.expect_punctuation("}", "`}`")?;
                (names, close.span.end)
            } else {
                let name = self.parse_imported_name("expected an imported name after `.`")?;
                let end = name.span.end;
                (vec![name], end)
            };

            if self.at_punctuation(".") {
                self.error_current("module paths use `/` between segments instead of `.`");
                return Err(());
            }

            return Ok(Item::FromImport(FromImportItem {
                span: self.span(start.span.start, end),
                visibility,
                path,
                names,
            }));
        }

        let end = path.span.end;
        let alias = default_namespace_alias(&path);

        Ok(Item::Import(ImportItem {
            span: self.span(start.span.start, end),
            visibility,
            path,
            alias,
            alias_is_default: true,
        }))
    }

    pub(super) fn parse_imported_name(&mut self, message: &str) -> ParseResult<ImportedName> {
        if self.at_punctuation("*") {
            self.error_current("wildcard imports are not supported");
            return Err(());
        }
        let name = self.expect_name_identifier(message)?;
        let mut end = name.span.end;
        let alias = if self.match_keyword(Keyword::As).is_some() {
            let alias = self.expect_name_identifier("expected import alias after `as`")?;
            end = alias.span.end;
            Some(ImportAlias {
                span: alias.span,
                name: alias.value,
            })
        } else {
            None
        };

        Ok(ImportedName {
            span: self.span(name.span.start, end),
            name_span: name.span,
            name: name.value,
            alias,
        })
    }

    pub(super) fn parse_module_path(&mut self) -> ParseResult<ModulePath> {
        let mut value = String::new();
        let mut segments = Vec::new();
        let mut segment_spans = Vec::new();
        let mut start = self.current().span.start;

        if let Some(slash) = self.match_punctuation("/") {
            start = slash.span.start;
            value.push('/');
        }

        while self.at_punctuation(".") {
            let dot = self.bump();
            if value.is_empty() {
                start = dot.span.start;
            }

            if self.match_punctuation("/").is_some() {
                value.push_str("./");
                segments.push(".".to_string());
                segment_spans.push(dot.span);
                break;
            }

            if let Some(second_dot) = self.match_punctuation(".") {
                self.expect_punctuation("/", "`/`")?;
                value.push_str("../");
                segments.push("..".to_string());
                segment_spans.push(self.span(dot.span.start, second_dot.span.end));
                continue;
            }

            self.error_current("expected `/` or `.` in relative module path");
            return Err(());
        }

        let first = self.expect_module_path_segment("expected module path segment")?;
        let mut end = first.span.end;
        segments.push(first.value);
        segment_spans.push(first.span);

        while self.match_punctuation("/").is_some() {
            let segment =
                self.expect_module_path_segment("expected module path segment after `/`")?;
            end = segment.span.end;
            segments.push(segment.value);
            segment_spans.push(segment.span);
        }

        let path_segments = segments
            .iter()
            .filter(|segment| segment.as_str() != "." && segment.as_str() != "..")
            .cloned()
            .collect::<Vec<_>>();
        value.push_str(&path_segments.join("/"));

        Ok(ModulePath {
            span: self.span(start, end),
            value,
            segments,
            segment_spans,
        })
    }
}

const MODULE_PATH_SEGMENT_DIAGNOSTIC: &str = "module path segments must be snake_case identifiers";

impl Parser<'_> {
    fn expect_module_path_segment(&mut self, message: &str) -> ParseResult<ParsedIdentifier> {
        let segment = self.expect_identifier(message)?;
        if is_valid_module_path_segment(&segment.value) {
            return Ok(segment);
        }

        self.error_at(segment.span, MODULE_PATH_SEGMENT_DIAGNOSTIC);
        Err(())
    }
}

fn is_valid_module_path_segment(segment: &str) -> bool {
    if segment == "_" {
        return false;
    }

    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !matches!(first, 'a'..='z' | '_') {
        return false;
    }

    chars.all(|char| matches!(char, 'a'..='z' | '0'..='9' | '_'))
}

fn default_namespace_alias(path: &ModulePath) -> ImportAlias {
    for (segment, span) in path.segments.iter().zip(path.segment_spans.iter()).rev() {
        if segment != "." && segment != ".." {
            return ImportAlias {
                span: *span,
                name: segment.clone(),
            };
        }
    }

    ImportAlias {
        span: path.span,
        name: path.value.clone(),
    }
}
