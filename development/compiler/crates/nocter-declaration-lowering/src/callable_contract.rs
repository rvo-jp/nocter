use nocter_model::CallableGuarantees;
use nocter_syntax::{NodeId, NodeKind, SyntaxTree, direct_node};

/// Projects authored callable modifiers into their one syntax-independent guarantee contract.
///
/// Declaration headers and structural callable types have different surrounding grammars, but the
/// modifiers have one semantic meaning. Keeping this projection here prevents either syntax path
/// from defining a second guarantee mapping. Non-callable nodes return `None`, so a consumer
/// cannot silently obtain the default contract from an invalid syntax identity.
#[must_use]
pub fn project_callable_guarantees(tree: &SyntaxTree, node: NodeId) -> Option<CallableGuarantees> {
    let kind = tree.node(node)?.kind();
    if !matches!(
        kind,
        NodeKind::FunctionDeclaration
            | NodeKind::InterfaceMethod
            | NodeKind::ConstructionFunction
            | NodeKind::LiteralDeclaration
            | NodeKind::InherentMethod
            | NodeKind::CoercionDeclaration
            | NodeKind::EqualityOperator
            | NodeKind::OrderingOperator
            | NodeKind::IndexOperator
            | NodeKind::ExpansionOperator
            | NodeKind::DropDeclaration
            | NodeKind::CallableType
            | NodeKind::OperatorPredicate
            | NodeKind::CoercionPredicate
            | NodeKind::ExpansionPredicate
    ) {
        return None;
    }
    let guarantees = if direct_node(tree, node, NodeKind::NoAllocationModifier).is_some() {
        CallableGuarantees::no_allocation()
    } else {
        CallableGuarantees::default()
    };
    let guarantees = if direct_node(tree, node, NodeKind::BlockingModifier).is_some() {
        guarantees.admit_blocking()
    } else {
        guarantees
    };
    let guarantees = if direct_node(tree, node, NodeKind::NoTrapModifier).is_some() {
        guarantees.no_trap()
    } else {
        guarantees
    };
    if direct_node(tree, node, NodeKind::CompileTimeModifier).is_some() {
        Some(guarantees.admit_compile_time_evaluation())
    } else {
        Some(guarantees)
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::{
        AllocationGuarantee, CompileTimeGuarantee, NonblockingGuarantee, TrapGuarantee,
    };
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{NodeKind, ParseGoal, SyntaxTree, parse};

    use super::project_callable_guarantees;

    #[test]
    fn declaration_and_structural_type_modifiers_share_one_projection() {
        let declaration = parse_text("const noalloc notrap blocking func work(): void\n");
        let declaration_root = declaration
            .nodes()
            .find_map(|(id, node)| (node.kind() == NodeKind::FunctionDeclaration).then_some(id))
            .unwrap();
        let structural = parse_text("type Work = const noalloc notrap blocking func(): void\n");
        let structural_root = structural
            .nodes()
            .find_map(|(id, node)| (node.kind() == NodeKind::CallableType).then_some(id))
            .unwrap();

        let declaration_contract =
            project_callable_guarantees(&declaration, declaration_root).unwrap();
        let structural_contract =
            project_callable_guarantees(&structural, structural_root).unwrap();
        assert_eq!(declaration_contract, structural_contract);
        assert_eq!(
            declaration_contract.allocation(),
            AllocationGuarantee::NoAllocation
        );
        assert_eq!(
            declaration_contract.nonblocking(),
            NonblockingGuarantee::Unspecified
        );
        assert_eq!(declaration_contract.trap(), TrapGuarantee::NoTrap);
        assert_eq!(
            declaration_contract.compile_time(),
            CompileTimeGuarantee::Evaluatable
        );
        assert_eq!(
            project_callable_guarantees(&declaration, declaration.root_id()),
            None
        );
    }

    fn parse_text(text: &str) -> SyntaxTree {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("callable-contract.nct"), text.as_bytes())
            .unwrap();
        parse(sources.get(source).unwrap(), ParseGoal::SourceFile)
    }
}
