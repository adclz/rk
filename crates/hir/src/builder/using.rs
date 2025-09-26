use ast::generated::UsingDirective;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNodeId;

use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::check::errors::analysis_error::AnalysisError;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::interned::namespace::NamespacePath;
use crate::hir_def::using::Using;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_using(
        &mut self,
        using: &'db UsingDirective,
    ) -> anyhow::Result<Vec<Using<'db>>, AnalysisError<'db>> {
        let mut result = vec![];
        for child in using.children.iter() {
            let mut path = vec![];
            for child in child.cast(self.ast).children.iter() {
                path.push(SpanIdent::new(self.db, self, child)?);
            }
            result.push(Using::new(
                self.db,
                NamespacePath::from((self.db, &path)),
                child.cast(self.ast).into(),
                self.current_scope,
            ));
        }
        Ok(result)
    }

    pub fn parse_usings(
        &mut self,
        usings: &[AstNodeId<UsingDirective>],
    ) -> anyhow::Result<Vec<Using<'db>>, AnalysisError<'db>> {
        let mut using = vec![];
        for directive in usings.iter() {
            for child in directive.cast(self.ast).children.iter() {
                let mut path = vec![];
                for child in child.cast(self.ast).children.iter() {
                    path.push(SpanIdent::new(self.db, self, child)?);
                }
                using.push(Using::new(
                    self.db,
                    NamespacePath::from((self.db, &path)),
                    child.cast(self.ast).into(),
                    self.current_scope,
                ));
            }
        }
        Ok(using)
    }
}
