use std::ops::Deref;

use crate::diagnostics::diagnostic_builder::diag;
use crate::diagnostics::DiagnosticAccumulator;
use crate::hir::interned::identifier::Ident;
use crate::hir::interned::namespace::SpannedNamespaceAccess;
use crate::hir::pous::class::Class;
use crate::hir::pous::pou::{Pou, PouDecl};
use crate::hir::scopes::scope::{PouId, Scope, ScopeId, ScopeKind, ScopedPouId, Visibility};
use crate::hir::visibility::Modifiers;
use crate::parser::semantic_index::{SemanticIndexBuilder};
use ast::generated::ClassDecl;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use salsa::Accumulator;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_class(&mut self, class: &ClassDecl) -> anyhow::Result<PouId> {
                let extends = class
            .extends
            .as_ref()
            .map(|e| SpannedNamespaceAccess::from_ast(self.db, self.file, e))
            .transpose()?;

        let implements = class
            .implements
            .as_ref()
            .map(|i| {
                i.children
                    .iter()
                    .map(|i| SpannedNamespaceAccess::from_ast(self.db, self.file, &i))
                    .collect()
            })
            .transpose()?;

        let mut modifiers = Modifiers::empty();
        class.qualifier.as_ref().map(|q| match q.deref() {
            ast::generated::Operators_2::Token_ABSTRACT(_) => modifiers.insert(Modifiers::ABSTRACT),
            ast::generated::Operators_2::Token_FINAL(_) => modifiers.insert(Modifiers::FINAL),
        });

        class.children.iter().for_each(|f| {
            type Error = ast::generated::ERRExtendsMultipleTimes_ERRImplementsBeforeExtends_ERRImplementsMultipleTimes;
            match f.deref() {
                Error::ERRExtendsMultipleTimes(err) => {
                    let diag = diag()
                        .message("EXTENDS can only be defined once".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), self.db);
                },
                Error::ERRImplementsBeforeExtends(err) => {
                    let diag = diag()
                        .message("IMPLEMENTS can only be defined after EXTENDS".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), self.db);
                },
                Error::ERRImplementsMultipleTimes(err) => {
                    let diag = diag()
                        .message("IMPLEMENTS can only be defined once".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), self.db);
                },
            } 
        });

        let id = ScopeId::from(class.get_id());
        let pou_key = PouId::from(class.get_id());
        let name = Ident::from_node(self.db, self.file, class.name.deref())?;
        let usings = self.parse_usings(&class.directives)?;

        let result =
            Class::new(self.db, extends, implements, modifiers, self.current_scope);

        let scope = Scope::new(
            self.file,
            ScopeKind::Pou(pou_key),
            usings,
            id,
            Visibility::empty(),
            Some(self.current_scope),
        );

        self.pou_keys.insert(
            PouId::from(class.get_id()),
            PouDecl::new(
                self.db,
                Pou::Class(result),
                class.get_span(),
                name,
                class.name.get_span(),
            ),
        );

        self.scope_to_pous.entry(id).or_default().insert(
                name.clone(),
                ScopedPouId(pou_key, self.file),
        );

        Ok(pou_key)
    }
}
