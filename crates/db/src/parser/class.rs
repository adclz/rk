use std::ops::Deref;

use crate::diagnostics::diagnostic_builder::diag;
use crate::diagnostics::DiagnosticAccumulator;
use crate::hir;
use crate::hir::visibility::Modifiers;
use crate::parser::namespace::ParseUsing;
use crate::parser::Parse;
use crate::solver::fq_name::SpannedPath;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::{file::File, BaseDatabase};
use salsa::Accumulator;

impl<'db> Parse<'db> for ast::generated::ClassDecl {
    type Output = hir::class::Class<'db>;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        let extends = self
            .extends
            .as_ref()
            .map(|e| SpannedPath::new(db, file, e))
            .transpose()?;

        let implements = self
            .implements
            .as_ref()
            .map(|i| {
                i.children
                    .iter()
                    .map(|i| SpannedPath::new(db, file, &i))
                    .collect()
            })
            .transpose()?;

        let mut modifiers = Modifiers::empty();
        self.qualifier.as_ref().map(|q| match q.deref() {
            ast::generated::Operators_2::Token_ABSTRACT(_) => modifiers.insert(Modifiers::ABSTRACT),
            ast::generated::Operators_2::Token_FINAL(_) => modifiers.insert(Modifiers::FINAL),
        });

        self.children.iter().for_each(|f| {
            type Error = ast::generated::ERRExtendsMultipleTimes_ERRImplementsBeforeExtends_ERRImplementsMultipleTimes;
            match f.deref() {
                Error::ERRExtendsMultipleTimes(err) => {
                    let diag = diag()
                        .file(file)
                        .message("EXTENDS can only be defined once".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), db);
                },
                Error::ERRImplementsBeforeExtends(err) => {
                    let diag = diag()
                        .file(file)
                        .message("IMPLEMENTS can only be defined after EXTENDS".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), db);
                },
                Error::ERRImplementsMultipleTimes(err) => {
                    let diag = diag()
                        .file(file)
                        .message("IMPLEMENTS can only be defined once".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), db);
                },
            } 
        });

        let using = self.directives.parse_using(db, file)?;

        Ok(hir::class::Class::new(
            db, extends, using, implements, modifiers,
        ))
    }
}
