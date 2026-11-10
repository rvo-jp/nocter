//! Freeze authored constant-reference edges through selected const callables, including dead source.
use super::super::{ExpressionKind, flow};
use nocter_constant_evaluation::state::{Dependency, Site};
use nocter_declarations::value_access::{CallableKind, Program};
use nocter_model::{AccessCallableId, CompileTimeGuarantee, ValueFunctionOrigin};
use std::collections::{BTreeMap, BTreeSet};
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
pub(super) struct Dependencies {
    pub(super) edges: BTreeMap<AccessCallableId, Box<[Dependency]>>,
    pub(super) sources: BTreeSet<AccessCallableId>,
}
pub(super) fn collect(
    program: &Program,
    bodies: &BTreeMap<AccessCallableId, flow::Query>,
) -> Result<Dependencies, flow::IntegrityError> {
    let mut edges = BTreeMap::new();
    let mut sources = BTreeSet::new();
    for (root, declaration) in program.callables().iter() {
        if declaration.kind() != CallableKind::ConstantInitializer {
            continue;
        }
        let mut found = BTreeMap::new();
        let mut visited = BTreeSet::new();
        let mut pending = vec![(root, Vec::new())];
        while let Some((code, path)) = pending.pop() {
            if !visited.insert(code) {
                continue;
            }
            sources.insert(code);
            let query = bodies.get(&code).ok_or(INVALID)?;
            for (id, expression) in query.typed().body().expressions().iter() {
                match &expression.kind {
                    ExpressionKind::Constant(initializer) => {
                        let mut through = path.clone();
                        through.push(Site::Expression {
                            code,
                            expression: id,
                        });
                        found.entry(*initializer).or_insert(Dependency {
                            initializer: *initializer,
                            through: through.into(),
                        });
                    }
                    ExpressionKind::Call {
                        target: ValueFunctionOrigin::Declaration(target),
                        ..
                    } => {
                        let declaration = program.callables().get(*target).ok_or(INVALID)?;
                        if !matches!(
                            declaration.kind(),
                            CallableKind::Function(CompileTimeGuarantee::Evaluatable)
                        ) {
                            return Err(INVALID);
                        }
                        let mut through = path.clone();
                        through.push(Site::Expression {
                            code,
                            expression: id,
                        });
                        pending.push((*target, through));
                    }
                    _ => {}
                }
            }
        }
        edges.insert(root, found.into_values().collect());
    }
    Ok(Dependencies { edges, sources })
}
