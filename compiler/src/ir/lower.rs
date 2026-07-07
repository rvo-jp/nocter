use super::{Function, Instruction, IrModule, Type};
use crate::analysis::CompileUnitAnalysis;
use crate::ast::{Expr, Item, ProgramDecl, Stmt, TypeExpr, UnaryOperator};
use crate::diagnostics::Diagnostic;

pub(crate) fn lower_program(analysis: &CompileUnitAnalysis) -> Result<IrModule, Vec<Diagnostic>> {
    let Some(root) = analysis.root_file() else {
        return Err(vec![Diagnostic::error(
            "E8000",
            "IR lowering requires a root source file",
        )]);
    };

    let Some(program) = root.ast.items.iter().find_map(|item| match item {
        Item::Program(program) => Some(program),
        _ => None,
    }) else {
        return Err(vec![Diagnostic::error(
            "E8000",
            "IR lowering requires a `program` entry",
        )]);
    };

    let function = lower_program_function(program)?;

    Ok(IrModule::new(vec![function]))
}

fn lower_program_function(program: &ProgramDecl) -> Result<Function, Vec<Diagnostic>> {
    let return_type = lower_program_return_type(&program.return_type)?;
    let instructions = lower_program_body(program, return_type)?;

    Ok(Function {
        name: "program".to_string(),
        return_type,
        instructions,
    })
}

fn lower_program_return_type(ty: &TypeExpr) -> Result<Type, Vec<Diagnostic>> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "void" => Ok(Type::Void),
        TypeExpr::Fallible(fallible) => lower_program_return_type(&fallible.success),
        _ => Err(vec![Diagnostic::error(
            "E8001",
            "IR v0 can only lower `program` return type `i32`, `i32!`, or `void`",
        )]),
    }
}

fn lower_program_body(
    program: &ProgramDecl,
    return_type: Type,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match program.body.statements.as_slice() {
        [Stmt::Return(statement)] => match (return_type, &statement.expression) {
            (Type::I32, Some(expression)) => {
                let value = lower_i32_literal(expression)?;
                Ok(vec![Instruction::ReturnI32(value)])
            }
            (Type::Void, None) => Ok(vec![Instruction::ReturnVoid]),
            (Type::Void, Some(_)) => Err(vec![Diagnostic::error(
                "E8002",
                "IR v0 cannot lower value returns from `void` program",
            )]),
            (Type::I32, None) => Err(vec![Diagnostic::error(
                "E8002",
                "IR v0 cannot lower bare returns from `i32` program",
            )]),
        },
        [] if return_type == Type::Void => Ok(vec![Instruction::ReturnVoid]),
        _ => Err(vec![Diagnostic::error(
            "E8002",
            "IR v0 can only lower `program` bodies containing `return <i32 literal>` or a void return",
        )]),
    }
}

fn lower_i32_literal(expression: &Expr) -> Result<i32, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_i32_literal(&literal.value),
        Expr::Unary(unary) if unary.operator == UnaryOperator::Negate => {
            let value = lower_unsigned_integer_literal(&unary.operand)?;

            if value == (i32::MAX as u32) + 1 {
                Ok(i32::MIN)
            } else {
                i32::try_from(value)
                    .map(|value| -value)
                    .map_err(|_| integer_out_of_range_diagnostic())
            }
        }
        Expr::Group(group) => lower_i32_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "IR v0 can only lower integer literal returns",
        )]),
    }
}

fn lower_unsigned_integer_literal(expression: &Expr) -> Result<u32, Vec<Diagnostic>> {
    match expression {
        Expr::IntegerLiteral(literal) => parse_u32_literal(&literal.value),
        Expr::Group(group) => lower_unsigned_integer_literal(&group.expression),
        _ => Err(vec![Diagnostic::error(
            "E8003",
            "IR v0 can only lower integer literal returns",
        )]),
    }
}

fn parse_i32_literal(text: &str) -> Result<i32, Vec<Diagnostic>> {
    let value = parse_u32_literal(text)?;
    i32::try_from(value).map_err(|_| integer_out_of_range_diagnostic())
}

fn parse_u32_literal(text: &str) -> Result<u32, Vec<Diagnostic>> {
    let (base, digits) = literal_base_and_digits(text);
    let digits = digits.replace('_', "");

    u32::from_str_radix(&digits, base).map_err(|_| integer_out_of_range_diagnostic())
}

fn literal_base_and_digits(text: &str) -> (u32, &str) {
    if let Some(digits) = text.strip_prefix("0x") {
        (16, digits)
    } else if let Some(digits) = text.strip_prefix("0b") {
        (2, digits)
    } else {
        (10, text)
    }
}

fn integer_out_of_range_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8003",
        "IR v0 integer literal return is outside the `i32` range",
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{CompileUnit, analyze_compile_unit};
    use crate::frontend::{FrontendOptions, load_compile_unit};
    use crate::source::SourceMap;
    use crate::target::DEFAULT_TARGET;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn lowers_program_returning_i32_literal() {
        let ir = lower_text(
            r#"program(): i32 {
    return 42
}
"#,
        );

        assert_eq!(
            ir,
            IrModule::new(vec![Function {
                name: "program".to_string(),
                return_type: Type::I32,
                instructions: vec![Instruction::ReturnI32(42)],
            }])
        );
    }

    #[test]
    fn lowers_program_returning_negative_i32_literal() {
        let ir = lower_text(
            r#"program(): i32 {
    return -42
}
"#,
        );

        assert_eq!(
            ir.functions[0].instructions,
            vec![Instruction::ReturnI32(-42)]
        );
    }

    #[test]
    fn lowers_void_program_with_empty_body() {
        let ir = lower_text(
            r#"program(): void {
}
"#,
        );

        assert_eq!(
            ir,
            IrModule::new(vec![Function {
                name: "program".to_string(),
                return_type: Type::Void,
                instructions: vec![Instruction::ReturnVoid],
            }])
        );
    }

    #[test]
    fn reports_unsupported_program_body() {
        let diagnostics = lower_text_diagnostics(
            r#"program(): i32 {
    let value = 1
    return value
}
"#,
        );

        assert_eq!(diagnostics[0].code, "E8002");
    }

    #[test]
    fn rejects_nested_negative_integer_literal() {
        let diagnostics = lower_text_diagnostics(
            r#"program(): i32 {
    return -(-42)
}
"#,
        );

        assert_eq!(diagnostics[0].code, "E8003");
    }

    fn lower_text(text: &str) -> IrModule {
        let diagnostics = lower_text_diagnostics(text);
        match diagnostics.as_slice() {
            [] => {
                let analysis = analyze_text(text);
                lower_program(&analysis).unwrap()
            }
            diagnostics => panic!("unexpected diagnostics: {diagnostics:?}"),
        }
    }

    fn lower_text_diagnostics(text: &str) -> Vec<Diagnostic> {
        let analysis = analyze_text(text);
        match lower_program(&analysis) {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics,
        }
    }

    fn analyze_text(text: &str) -> crate::analysis::CompileUnitAnalysis {
        let mut sources = SourceMap::new();
        let source = sources.add_source("app.nct", None, text);
        let temp_root = make_temp_project();
        let nocter_home = make_nocter_home(&temp_root);
        let unit: CompileUnit = load_compile_unit(
            &mut sources,
            source,
            &FrontendOptions {
                nocter_home: Some(nocter_home),
                target: DEFAULT_TARGET.to_string(),
            },
        )
        .unwrap();
        let analysis = analyze_compile_unit(&sources, &unit);
        let diagnostics = analysis.diagnostics();
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        analysis
    }

    fn make_temp_project() -> PathBuf {
        let unique = format!(
            "nocter-ir-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn make_nocter_home(root: &Path) -> PathBuf {
        let home = root.join(".nocter");
        fs::create_dir_all(home.join("std")).unwrap();
        fs::create_dir_all(home.join("targets/arm64-darwin/std")).unwrap();
        fs::write(home.join("std/prelude.nct"), "").unwrap();
        home
    }
}
