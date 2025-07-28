use std::{ops::Deref, sync::Arc};

use ast::generated::UsingDirective;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

use crate::hir::interned::identifier::SpannedIdent;
use crate::hir::interned::namespace::NamespacePath;
use crate::hir::using::Using;
use crate::parser::semantic_index::SemanticIndexBuilder;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_using(&mut self, using: &'db UsingDirective) -> anyhow::Result<Vec<Using<'db>>> {
        let mut result = vec![];
        for child in using.children.iter() {
            let mut path = vec![];
            for child in child.children.iter() {
                path.push(SpannedIdent::new(self.db, self.file, child.deref())?);
            }
            result.push(Using::new(
                self.db,
                NamespacePath::from((self.db, &path)),
                child.get_span(),
                self.current_scope,
            ));
        }
        Ok(result)
    }

    pub fn parse_usings(
        &mut self,
        usings: &[Arc<UsingDirective>],
    ) -> anyhow::Result<Vec<Using<'db>>> {
        let mut using = vec![];
        for directive in usings.iter() {
            for child in directive.children.iter() {
                let mut path = vec![];
                for child in child.children.iter() {
                    path.push(SpannedIdent::new(self.db, self.file, child.deref())?);
                }
                using.push(Using::new(
                    self.db,
                    NamespacePath::from((self.db, &path)),
                    child.get_span(),
                    self.current_scope,
                ));
            }
        }
        Ok(using)
    }
}
