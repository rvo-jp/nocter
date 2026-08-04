use super::*;
use crate::ast::{ClosureCallableCapability, ClosureTypeExpr};

pub(in crate::typecheck) fn closure_callable_contract_diagnostic(
    sources: &SourceMap,
    argument_span: ByteSpan,
    closure: &ClosureTypeExpr,
    bound: &Type,
    expected_capability: ClosureCallableCapability,
    bound_span: ByteSpan,
) -> Diagnostic {
    let reason = if closure.capability > expected_capability {
        match closure.capability {
            ClosureCallableCapability::Consuming => {
                "its body consumes captured state and therefore requires a consuming callback"
            }
            ClosureCallableCapability::Readwrite => {
                "its body mutates captured state and therefore requires a mutable callback"
            }
            ClosureCallableCapability::Readonly => unreachable!(),
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
        ClosureCallableCapability::Consuming => {
            "use `CallOnce`, or stop moving an owned capture from the closure body".to_string()
        }
        ClosureCallableCapability::Readwrite => {
            "use `CallMut`, or make the closure body readonly".to_string()
        }
        ClosureCallableCapability::Readonly => {
            "align the closure parameter and result annotations with the callable contract"
                .to_string()
        }
    });
    diagnostic
}
