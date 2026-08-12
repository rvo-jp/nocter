use crate::ast::Item;
use crate::resolve::ResolveOutput;
use crate::semantics::{InterpolationRuntimeInput, RuntimeCallableInput};

pub(super) fn append_test_format_contract(text: &str) -> String {
    format!(
        r#"{text}

pub interface Format {{
    pub method &self.format_into(output: &+String): void
}}

conform Format for str {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for String {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for bool {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for i8 {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for i16 {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for i32 {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for i64 {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for isize {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for u8 {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for u16 {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for u32 {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for u64 {{ method &self.format_into(output: &+String): void {{ return }} }}
conform Format for usize {{ method &self.format_into(output: &+String): void {{ return }} }}
"#,
    )
}

pub(super) fn attach_test_format_runtime(ast: &crate::ast::AstFile, resolved: &mut ResolveOutput) {
    crate::resolve::attach_test_builtin_conformances(resolved, ast);
    let string_span = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(struct_) if struct_.name == "String" => Some(struct_.span),
            _ => None,
        })
        .expect("expected test String");
    let (interface_span, method_span) = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Interface(interface) if interface.name == "Format" => interface
                .methods
                .iter()
                .find(|method| method.name == "format_into")
                .map(|method| (interface.name_span, method.name_span)),
            _ => None,
        })
        .expect("expected test Format interface");
    resolved
        .trusted_declarations
        .set_interpolation_runtime(InterpolationRuntimeInput::new(
            string_span,
            RuntimeCallableInput {
                declaration: string_span,
                target_name: "test".to_string(),
            },
            interface_span,
            "Format".to_string(),
            method_span,
            "format_into".to_string(),
        ));
}
