//! Condition-only equivalence for fixed-target closures with supported direct callable data.
use super::{INVALID, flow};
use crate::value_access::{Query, evaluation};
use nocter_declarations::value_access::{CodeInstanceKey, Program};
use nocter_model::ValueCode;
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::value_access) struct Traversal {
    fixed: BTreeSet<ValueCode>,
    supported: BTreeSet<ValueCode>,
    exact: BTreeSet<CodeInstanceKey>,
    domain: evaluation::Types,
}
impl Traversal {
    pub(super) fn new(scope: &BTreeMap<ValueCode, &Query>) -> Self {
        Self {
            fixed: evaluation::call_support::witness_independent(scope),
            supported: BTreeSet::new(),
            exact: BTreeSet::new(),
            domain: evaluation::Types::default(),
        }
    }
    /// No witness in a fixed-target closure selects an invoked body. Its remaining bound
    /// clauses ask only whether direct callable data is supported, including actual cleanup.
    /// All-supported frames therefore have equal conditions: fixed clauses are unchanged,
    /// substituted direct leaves are supported, and forwarding preserves that support under
    /// the declaration authority's view normalization. Check one complete representative and
    /// all its outgoing edges. An unsupported frame retains its exact identity and local cause.
    ///
    /// This proof is specific to the present condition vocabulary. It supplies no equal layout,
    /// environment, executable plan, final serialization or source-generation certificate.
    pub(in crate::value_access) fn visit(
        &mut self,
        program: &Program,
        key: &CodeInstanceKey,
    ) -> Result<bool, flow::IntegrityError> {
        if key.generic_application().is_some()
            || key.has_compound_input_contexts()
            || key.environment().is_some()
            || key
                .witnesses()
                .values()
                .any(|witness| witness.environment().is_some())
        {
            return Ok(self.exact.insert(key.clone()));
        }
        if self.fixed.contains(&key.code()) {
            let mut supported = true;
            for witness in key.witnesses().values() {
                let data = program.function_data(witness.witness()).ok_or(INVALID)?;
                supported &= self.domain.supports(program, data)?;
            }
            if supported {
                return Ok(self.supported.insert(key.code()));
            }
        }
        Ok(self.exact.insert(key.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_access::{
        ExpressionKind,
        accepted::SourceUnit,
        tests::{bound, input},
    };
    use nocter_declarations::value_access::FunctionWitness;
    use nocter_model::ValueFunctionOrigin;

    #[test]
    fn fixed_target_condition_sharing_does_not_forget_received_compound_contexts() {
        let (program, frames) = crate::value_access::evaluation::received_test_frames();
        assert_ne!(frames[0], frames[1]);
        assert!(frames.iter().all(|frame| frame.environment().is_none()
            && frame.witnesses().is_empty()
            && frame.has_compound_input_contexts()));
        let mut traversal = Traversal {
            fixed: BTreeSet::from([frames[0].code()]),
            supported: BTreeSet::new(),
            exact: BTreeSet::new(),
            domain: evaluation::Types::default(),
        };
        for frame in &frames {
            assert!(traversal.visit(&program, frame).unwrap());
        }
        for frame in &frames {
            assert!(!traversal.visit(&program, frame).unwrap());
        }
        assert!(traversal.supported.is_empty());
        assert_eq!(traversal.exact, frames.into());
    }

    #[test]
    fn only_supported_fixed_target_conditions_share_and_missing_bodies_remain_conservative() {
        let source = "func first(value: i32): i32 { value }\nfunc second(value: i32): i32 { value + 1 }\nconst func ignore(action: func(i32): i32): i32 { 42 }\nconst func forward(action: func(i32): i32): i32 { ignore(action) }\nconst func invoke(action: func(i32): i32): i32 { action(0) }\nfunc run(take input: error): void { let number = 40\nlet good = (number; value: i32): i32 { value + number }\nlet bad = (input^; value: i32): i32 { value }\nlet _ = forward(good)\nlet _ = forward(bad) }";
        let (sources, tree) = input(source);
        let unit = SourceUnit::prepare(bound(&sources, &tree))
            .unwrap()
            .into_unit();
        let program = unit.program();
        let named = |name| {
            program
                .resolve_code(program.symbols().get(name).unwrap())
                .unwrap()
        };
        let mut scope = unit
            .bodies()
            .map(|(_, body)| (body.typed().code(), body.typed()))
            .chain(
                unit.closures()
                    .iter()
                    .map(|(&code, body)| (ValueCode::Closure(code), body.typed())),
            )
            .collect::<BTreeMap<_, _>>();
        let run = scope[&ValueCode::Declaration(named("run"))];
        let captures = run
            .body()
            .expressions()
            .iter()
            .filter_map(|(_, expression)| match expression.kind {
                ExpressionKind::Closure { closure: code, .. } => Some(FunctionWitness {
                    origin: ValueFunctionOrigin::Closure(code),
                    invocation: nocter_model::ValueInvocation::Read,
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(captures.len(), 2);
        let key = |name, witness| {
            let code = named(name);
            let formal = program.codes().get(code).unwrap().parameters()[0];
            program
                .bind_code(code.into(), [(formal.into(), witness)])
                .unwrap()
        };
        let first = FunctionWitness::from(named("first"));
        let second = FunctionWitness::from(named("second"));
        let mut traversal = Traversal::new(&scope);
        assert!(traversal.visit(program, &key("forward", first)).unwrap());
        assert!(!traversal.visit(program, &key("forward", second)).unwrap());
        assert!(
            !traversal
                .visit(program, &key("forward", captures[0]))
                .unwrap()
        );
        // The captured error is not a supported environment, despite the same input signature.
        let unsupported = key("forward", captures[1]);
        assert!(traversal.visit(program, &unsupported).unwrap());
        assert!(!traversal.visit(program, &unsupported).unwrap());
        assert!(traversal.visit(program, &key("invoke", first)).unwrap());
        assert!(traversal.visit(program, &key("invoke", second)).unwrap());
        scope.remove(&ValueCode::Declaration(named("ignore")));
        let mut incomplete = Traversal::new(&scope);
        assert!(incomplete.visit(program, &key("forward", first)).unwrap());
        assert!(incomplete.visit(program, &key("forward", second)).unwrap());
    }
}
