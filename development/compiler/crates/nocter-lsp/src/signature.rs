use nocter_json::{Member, Value};

use crate::TextDocumentPositionParams;

pub type SignatureHelpParams = TextDocumentPositionParams;

/// One UTF-16 offset range within a signature label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureParameter {
    start: u32,
    end: u32,
}

impl SignatureParameter {
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

/// Renders one compiler-selected signature using the protocol's single active-signature shape.
#[must_use]
pub fn signature_help_result(
    label: &str,
    parameters: &[SignatureParameter],
    active_parameter: Option<u32>,
) -> Value {
    let mut members = vec![
        Member {
            name: "signatures".into(),
            value: Value::Array(vec![Value::Object(vec![
                Member {
                    name: "label".into(),
                    value: Value::String(label.into()),
                },
                Member {
                    name: "parameters".into(),
                    value: Value::Array(
                        parameters
                            .iter()
                            .map(|parameter| {
                                Value::Object(vec![Member {
                                    name: "label".into(),
                                    value: Value::Array(vec![
                                        Value::Number(parameter.start.to_string().into()),
                                        Value::Number(parameter.end.to_string().into()),
                                    ]),
                                }])
                            })
                            .collect(),
                    ),
                },
            ])]),
        },
        Member {
            name: "activeSignature".into(),
            value: Value::Number("0".into()),
        },
    ];
    if let Some(active_parameter) = active_parameter {
        members.push(Member {
            name: "activeParameter".into(),
            value: Value::Number(active_parameter.to_string().into()),
        });
    }
    Value::Object(members)
}

#[cfg(test)]
mod tests {
    use nocter_json::write_value;

    use super::{SignatureParameter, signature_help_result};

    #[test]
    fn renders_one_selected_signature_and_optional_active_parameter() {
        let mut rendered = String::new();
        write_value(
            &mut rendered,
            &signature_help_result(
                "func add(left: i32, right: i32): i32",
                &[
                    SignatureParameter::new(9, 18),
                    SignatureParameter::new(20, 30),
                ],
                Some(1),
            ),
        );
        assert_eq!(
            rendered,
            concat!(
                "{\"signatures\":[{\"label\":",
                "\"func add(left: i32, right: i32): i32\",",
                "\"parameters\":[{\"label\":[9,18]},{\"label\":[20,30]}]}],",
                "\"activeSignature\":0,\"activeParameter\":1}"
            )
        );
    }
}
