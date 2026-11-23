//! Resolve one namespace/declaration prefix; remaining components use the ordinary field checker.
use super::{Failure, Rule, checker::Checker};
use nocter_declarations::value_access::{
    BehaviorOwner, DeclarationId, Namespace, QualifiedNameError,
};
use nocter_model::{AccessCodeId, Symbol};
use nocter_syntax::{NodeId, NodeKind, SyntaxToken, TokenKind};

pub(super) struct QualifiedSelection {
    pub code: AccessCodeId,
    pub token: SyntaxToken,
    pub node: NodeId,
    pub members: Vec<SyntaxToken>,
}
impl Checker<'_, '_> {
    fn path_symbols(&self, path: &[SyntaxToken]) -> Result<Vec<Option<Symbol>>, Failure> {
        path.iter()
            .map(|&token| {
                super::source_origin(self.syntax(), nocter_syntax::SyntaxOrigin::Token(token))?;
                Ok(self.program().symbols().get(self.text(token)?))
            })
            .collect()
    }
    fn reject_path<T>(
        &self,
        namespace: Namespace,
        path: &[SyntaxToken],
        error: QualifiedNameError,
    ) -> Result<T, Failure> {
        let (position, rule) = match error {
            QualifiedNameError::UnknownScope | QualifiedNameError::EmptyPath => {
                return Err(Failure::InconsistentSyntax);
            }
            QualifiedNameError::UnknownNamespace { position } => (position, Rule::UnknownNamespace),
            QualifiedNameError::UnknownDeclaration { position } => (
                position,
                if namespace == Namespace::Type {
                    Rule::UnknownType
                } else {
                    Rule::UnknownName
                },
            ),
            QualifiedNameError::Inaccessible { position } => (position, Rule::InaccessibleName),
            QualifiedNameError::NamespaceOnly { position } => (
                position,
                if namespace == Namespace::Type {
                    Rule::UnknownType
                } else {
                    Rule::NamespaceValue
                },
            ),
        };
        self.reject(
            rule,
            *path.get(position).ok_or(Failure::InconsistentSyntax)?,
        )
    }
    pub(super) fn declaration_path(
        &self,
        namespace: Namespace,
        path: &[SyntaxToken],
    ) -> Result<DeclarationId, Failure> {
        let symbols = self.path_symbols(path)?;
        let scope = self.scope_at(*path.first().ok_or(Failure::InconsistentSyntax)?)?;
        match self
            .program()
            .declaration_path_in(scope, namespace, &symbols)
        {
            Ok(declaration) => Ok(declaration),
            Err(error) => self.reject_path(namespace, path, error),
        }
    }
    pub(super) fn require_declaration(
        &self,
        declaration: DeclarationId,
        token: SyntaxToken,
    ) -> Result<(), Failure> {
        if self.program().header_ready(declaration) {
            Ok(())
        } else {
            Err(Failure::RequiredHeader {
                declaration,
                origin: nocter_syntax::SyntaxOrigin::Token(token),
            })
        }
    }
    pub(super) fn name_path(
        &self,
        node: NodeId,
    ) -> Result<Option<Vec<(NodeId, SyntaxToken)>>, Failure> {
        let mut current = self.unwrap(node)?;
        let mut segments = Vec::new();
        loop {
            if self.kind(current)? == NodeKind::ReferenceExpression {
                segments.push((current, self.token(current)?));
                break;
            }
            let children = self.children(current);
            let [base, suffix] = children.as_slice() else {
                return Ok(None);
            };
            if self.kind(current)? != NodeKind::PostfixExpression
                || self.kind(*suffix)? != NodeKind::MemberSuffix
            {
                return Ok(None);
            }
            segments.push((current, self.member_token(*suffix)?));
            current = self.unwrap(*base)?;
        }
        segments.reverse();
        Ok(Some(segments))
    }
    pub(super) fn qualified_selection(
        &self,
        node: NodeId,
    ) -> Result<Option<QualifiedSelection>, Failure> {
        let Some(segments) = self.name_path(node)? else {
            return Ok(None);
        };
        let &(_, root) = segments.first().ok_or(Failure::InconsistentSyntax)?;
        let scope = self.scope_at(root)?;
        if self.lookup(self.text(root)?).is_some()
            || !self
                .program()
                .symbols()
                .get(self.text(root)?)
                .is_some_and(|name| self.program().has_namespace_in(scope, name))
        {
            return Ok(None);
        }
        let tokens = segments.iter().map(|&(_, token)| token).collect::<Vec<_>>();
        // Non-identifier suffixes belong to ordinary tuple fields after a selected declaration.
        let symbols = self.path_symbols(&tokens)?;
        let (declaration, consumed) =
            match self
                .program()
                .declaration_prefix_in(scope, Namespace::Value, &symbols)
            {
                Ok(result) => result,
                Err(error) => return self.reject_path(Namespace::Value, &tokens, error),
            };
        let DeclarationId::Code(code) = declaration else {
            return Err(Failure::InconsistentSyntax);
        };
        let &(node, token) = segments
            .get(consumed - 1)
            .ok_or(Failure::InconsistentSyntax)?;
        if token.kind() != TokenKind::Identifier {
            return Err(Failure::InconsistentSyntax);
        }
        self.require_declaration(declaration, token)?;
        Ok(Some(QualifiedSelection {
            code,
            token,
            node,
            members: tokens[consumed..].to_vec(),
        }))
    }

    pub(super) fn constructor_owner(&self, node: NodeId) -> Result<Option<BehaviorOwner>, Failure> {
        let Some(segments) = self.name_path(node)? else {
            return Ok(None);
        };
        let tokens: Vec<_> = segments.into_iter().map(|(_, token)| token).collect();
        let root = *tokens.first().ok_or(Failure::InconsistentSyntax)?;
        if self.lookup(self.text(root)?).is_some()
            || tokens
                .iter()
                .any(|token| token.kind() != TokenKind::Identifier)
        {
            return Ok(None);
        }
        let scope = self.scope_at(root)?;
        let symbols = self.path_symbols(&tokens)?;
        if self
            .program()
            .declaration_prefix_in(scope, Namespace::Value, &symbols)
            .is_ok()
        {
            return Ok(None);
        }
        if let [name] = tokens.as_slice() {
            return Ok(self.program().type_owner_named_in(scope, self.text(*name)?));
        }
        match self
            .program()
            .declaration_path_in(scope, Namespace::Type, &symbols)
        {
            Ok(DeclarationId::Record(owner)) => Ok(Some(BehaviorOwner::Record(owner))),
            Ok(DeclarationId::Code(_)) => Err(Failure::InconsistentSyntax),
            Err(error @ QualifiedNameError::Inaccessible { position })
                if position + 1 == tokens.len() =>
            {
                self.reject_path(Namespace::Type, &tokens, error)
            }
            Err(_) => Ok(None),
        }
    }
}
