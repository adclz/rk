use std::ops::Deref;

use crate::diagnostics::diagnostic_builder::diag;
use crate::diagnostics::DiagnosticAccumulator;
use crate::hir;
use crate::ident::Ident;
use crate::parser::namespace::ParseUsing;
use crate::parser::{Parse, ParseSpec, ParseVarSection};
use crate::solver::fq_name::SpannedPath;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::{file::File, BaseDatabase};
use salsa::Accumulator;

impl<'db> Parse<'db> for ast::generated::InterfaceDecl {
    type Output = hir::interface::Interface<'db>;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        let extends = self
            .extends
            .as_ref()
            .map(|i| {
                i.children
                    .iter()
                    .map(|i| SpannedPath::new(db, file, i))
                    .collect()
            })
            .transpose()?;

        let methods = self
            .prototype
            .iter()
            .map(|m| m.parse(db, file))
            .collect::<anyhow::Result<Vec<_>>>()?;

        self.children.iter().for_each(|f| {
            let diag = diag()
                .message("EXTENDS can only be defined once".into())
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .range(f.get_span())
                .call();
            DiagnosticAccumulator::accumulate(diag.into(), db);
        });

        let using = self.directives.parse_using(db, file)?;

        Ok(hir::interface::Interface::new(db, extends, using, methods))
    }
}

impl<'db> Parse<'db> for ast::generated::MethodPrototype {
    type Output = hir::interface::Method<'db>;

    fn parse(&'db self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        let name = Ident::from_node(db, file, &*self.name)?;
        let return_type = self
            .data_type
            .as_ref()
            .map(|i| match i.deref() {
                ast::generated::DataTypeAccess::ElemTypeName(elem_type_name) => {
                    elem_type_name.to_spec(db, file)
                }
                ast::generated::DataTypeAccess::NamespaceAccess(target) => target.to_spec(db, file),
            })
            .transpose()?;

        let mut variables = vec![];
        for variable in self.variables.iter() {
            match variable.deref() {
                ast::generated::InOutDecls_InputDecls_OutputDecls::InputDecls(decls) => {
                    decls.parse(db, file, &mut variables)?
                }
                ast::generated::InOutDecls_InputDecls_OutputDecls::InOutDecls(decls) => {
                    decls.parse(db, file, &mut variables)?
                }
                ast::generated::InOutDecls_InputDecls_OutputDecls::OutputDecls(decls) => {
                    decls.parse(db, file, &mut variables)?
                }
            }
        }

        Ok(hir::interface::Method::new(
            db,
            self.get_span(),
            name,
            self.name.get_span(),
            return_type,
            variables,
            self.get_id(),
        ))
    }
}
