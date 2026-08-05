use super::*;
use crate::ast::{CallableCapability, ClosureTypeExpr};

pub(in crate::typecheck) fn closure_callable_contract_diagnostic(
    sources: &SourceMap,
    argument_span: ByteSpan,
    closure: &ClosureTypeExpr,
    bound: &Type,
    expected_capability: CallableCapability,
    bound_span: ByteSpan,
) -> Diagnostic {
    let reason = if closure.capability > expected_capability {
        match closure.capability {
            CallableCapability::Consuming => {
                "its body consumes captured state and therefore requires a consuming callback"
            }
            CallableCapability::Readwrite => {
                "its body mutates captured state and therefore requires a mutable callback"
            }
            CallableCapability::Readonly => unreachable!(),
        }
    } else {
        "its parameter or result type does not match the callable contract"
    };
    let mut diagnostic = Diagnostic::error(
        "E0453",
        format!(
            "closure does not satisfy callable contract `{}` because {reason}",
            bound.display()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(argument_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(bound_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "the callable contract is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(match closure.capability {
        CallableCapability::Consuming => {
            "use a `func(...): ...` bound, or stop moving an owned capture from the closure body"
                .to_string()
        }
        CallableCapability::Readwrite => {
            "use an `&+func(...): ...` bound, or make the closure body readonly".to_string()
        }
        CallableCapability::Readonly => {
            "align the closure parameter and result annotations with the callable contract"
                .to_string()
        }
    });
    diagnostic
}
