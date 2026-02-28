use ast::generated::UsingDirective;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNodeId;

use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::hir_def::hir_node::HirNode;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::interned::namespace::SpanNamespacePath;
use crate::hir_def::using::Using;
use ide_diagnostic::IdeDiagnostic;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_using(
        &mut self,
        using: &'db UsingDirective,
    ) -> anyhow::Result<Vec<Using<'db>>, IdeDiagnostic> {
        let mut result = vec![];
        for child in using.children.iter() {
            let mut path = vec![];
            for child in child.cast(self.ast).children.iter() {
                path.push(SpanIdent::new(self.db, self, child)?);
            }
            let u = Using::new(
                self.db,
                SpanNamespacePath::from((self.db, &path, self.current_scope)),
                child.cast(self.ast).into(),
                self.current_scope,
            );
            self.register_node(child.cast(self.ast).into(), HirNode::Using(u));
            result.push(u);
        }
        Ok(result)
    }

    pub fn parse_usings(
        &mut self,
        usings: &[AstNodeId<UsingDirective>],
    ) -> anyhow::Result<Vec<Using<'db>>, IdeDiagnostic> {
        let mut using = vec![];
        for directive in usings.iter() {
            for child in directive.cast(self.ast).children.iter() {
                let mut path = vec![];
                for child in child.cast(self.ast).children.iter() {
                    path.push(SpanIdent::new(self.db, self, child)?);
                }
                let u = Using::new(
                    self.db,
                    SpanNamespacePath::from((self.db, &path, self.current_scope)),
                    child.cast(self.ast).into(),
                    self.current_scope,
                );
                self.register_node(child.cast(self.ast).into(), HirNode::Using(u));
                using.push(u);
            }
        }
        Ok(using)
    }
}
