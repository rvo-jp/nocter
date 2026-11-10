//! Source rejection before unsupported payload shapes can enter accepted body/flow products.
use super::{Failure, Rule, Site, checker::Checker, ir::LocalOrigin};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::ValueFunctionOrigin;
use nocter_syntax::SyntaxOrigin;
use std::collections::BTreeSet;

impl Checker<'_, '_> {
    pub(super) fn admit_body(&self) -> Result<(), Failure> {
        let source = self.source.bind(self.context)?;
        let result_origin = SyntaxOrigin::Node(source.result_origin());
        let body_origin = SyntaxOrigin::Node(source.body());
        let mut roots = vec![(self.result, result_origin)];
        for (id, local) in self.locals.iter() {
            let origin = match local.origin {
                LocalOrigin::Parameter(parameter) => {
                    let parameter = self
                        .program()
                        .parameters()
                        .get(parameter)
                        .ok_or(Failure::InconsistentSyntax)?;
                    SyntaxOrigin::Node(
                        source
                            .parameter_type(parameter.position())
                            .ok_or(Failure::InconsistentSyntax)?,
                    )
                }
                _ => match self.origins.locals.get(&id) {
                    Some(Site::Body(site)) => self
                        .projection
                        .resolve(*site)
                        .ok_or(Failure::InconsistentSyntax)?,
                    _ => body_origin,
                },
            };
            roots.push((local.data, origin));
        }
        for (id, expression) in self.expressions.iter() {
            if let Some(access) = expression.ty.access() {
                let Some(Site::Body(site)) = self.origins.expressions.get(&id) else {
                    return Err(Failure::InconsistentSyntax);
                };
                roots.push((
                    access.data(),
                    self.projection
                        .resolve(*site)
                        .ok_or(Failure::InconsistentSyntax)?,
                ));
            }
        }
        let mut seen = BTreeSet::new();
        let mut pending = Vec::new();
        for (data, origin) in roots {
            pending.push(data);
            while let Some(data) = pending.pop() {
                if !seen.insert(data) {
                    continue;
                }
                match self
                    .program()
                    .data_schema(data)
                    .ok_or(Failure::InconsistentSyntax)?
                {
                    PayloadSchema::Array(_) => {
                        return self.reject_message(
                            Rule::UnsupportedData,
                            origin,
                            "fixed-array value operations are outside this private body fragment"
                                .into(),
                            Vec::new(),
                        );
                    }
                    PayloadSchema::Builtin(_) => {}
                    PayloadSchema::Aggregate(schema) => {
                        pending.extend(schema.components().map(|component| component.data));
                    }
                    PayloadSchema::Function(witness) => {
                        let contract = self
                            .program()
                            .function_contract(witness)
                            .ok_or(Failure::InconsistentSyntax)?;
                        pending.push(contract.result);
                        pending.extend(contract.inputs.iter().map(|input| input.data));
                        if let ValueFunctionOrigin::Closure(id) = witness.origin {
                            pending.push(
                                self.program()
                                    .closure(id)
                                    .ok_or(Failure::InconsistentSyntax)?
                                    .environment(),
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
