use nocter_source::{ByteOffset, LineColumn, SourceFile, SourceMap, TextRange};

use crate::{DiagnosticOrigin, DiagnosticRenderError};

pub(crate) struct ProjectedOrigin<'a> {
    pub(crate) source: &'a SourceFile,
    pub(crate) range: TextRange,
    pub(crate) start: LineColumn,
    pub(crate) end: LineColumn,
}

pub(crate) fn project_origin(
    origin: DiagnosticOrigin,
    sources: &SourceMap,
) -> Result<ProjectedOrigin<'_>, DiagnosticRenderError> {
    let source = sources
        .get(origin.source())
        .ok_or(DiagnosticRenderError::UnknownSource(origin.source()))?;
    let range = origin.span().range();
    validate_range(source, range)?;
    let start = line_column(source, range, range.start())?;
    let end = line_column(source, range, range.end())?;
    Ok(ProjectedOrigin {
        source,
        range,
        start,
        end,
    })
}

fn line_column(
    source: &SourceFile,
    range: TextRange,
    offset: ByteOffset,
) -> Result<LineColumn, DiagnosticRenderError> {
    source
        .lines()
        .line_column(offset)
        .ok_or(DiagnosticRenderError::InvalidRange {
            source: source.id(),
            range,
        })
}

fn validate_range(source: &SourceFile, range: TextRange) -> Result<(), DiagnosticRenderError> {
    if range.end() > source.len() {
        return Err(DiagnosticRenderError::InvalidRange {
            source: source.id(),
            range,
        });
    }
    for offset in [range.start(), range.end()] {
        let index = usize::try_from(offset.get()).expect("source offsets fit usize");
        if !source.text().is_char_boundary(index) {
            return Err(DiagnosticRenderError::InvalidUtf8Boundary {
                source: source.id(),
                offset,
            });
        }
    }
    Ok(())
}

pub(crate) fn absolute_source_name(source: &SourceFile) -> Option<&str> {
    std::path::Path::new(source.name().as_str())
        .is_absolute()
        .then(|| source.name().as_str())
}
