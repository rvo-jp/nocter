//! Stable identities and compiler-owned process-entry selection for native tests.

use crate::ast::{AstFile, Item, TestDecl};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TestDeclarationId {
    item_index: usize,
    name: String,
}

impl TestDeclarationId {
    pub(crate) fn new(item_index: usize, name: impl Into<String>) -> Self {
        Self {
            item_index,
            name: name.into(),
        }
    }

    pub(crate) fn item_index(&self) -> usize {
        self.item_index
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn resolve<'a>(&self, ast: &'a AstFile) -> Option<&'a TestDecl> {
        match ast.items.get(self.item_index) {
            Some(Item::Test(test)) if test.name == self.name => Some(test),
            _ => None,
        }
    }
}

pub(crate) fn declared_tests(ast: &AstFile) -> Vec<TestDeclarationId> {
    ast.items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match item {
            Item::Test(test) => Some(TestDeclarationId::new(index, test.name.clone())),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TestRunId {
    target: crate::package::TestTargetId,
    declaration: TestDeclarationId,
}

impl TestRunId {
    pub(crate) fn new(
        target: crate::package::TestTargetId,
        declaration: TestDeclarationId,
    ) -> Self {
        Self {
            target,
            declaration,
        }
    }

    pub(crate) fn target(&self) -> &crate::package::TestTargetId {
        &self.target
    }

    pub(crate) fn declaration(&self) -> &TestDeclarationId {
        &self.declaration
    }
}
