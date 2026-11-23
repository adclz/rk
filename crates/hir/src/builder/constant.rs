// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::{
    Qualifier,
    builder::{ParseSpecInit, semantic_index::SemanticIndexBuilder},
    check::errors::{ToIdeDiagnostic, e00_syntax::SyntaxError},
    hir_def::{
        interned::identifier::Ident,
        pous::{pou::Pou, variable::VariableKind},
        scope::ScopeKind,
    },
};
use ast::generated::ConstantVarDecls;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    /// The constants of a `VAR_CONSTANT` section, each an item of the
    /// namespace it is written in, as each TYPE of a `TYPE` block is, with a
    /// scope of its own: what its type and its value are read in.
    pub fn parse_constants(&mut self, section: &ConstantVarDecls) -> Vec<Pou<'db>> {
        let mut constants = vec![];
        for child in section.children.iter() {
            match child.cast(self.ast) {
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::ERRVariableWithNoSpec(
                    child,
                ) => {
                    self.errors.push(
                        SyntaxError::MissingVarType(child.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::VarDeclInitList(
                    declaration,
                ) => {
                    for name in declaration.variables.cast(self.ast).children.iter() {
                        let previous_scope = self.current_scope;
                        let scope_id = self.generate_scope_id();
                        self.current_scope = scope_id;
                        let parsed = Ident::from_node(self.db, self.file, name.cast(self.ast))
                            .and_then(|ident| {
                                let spec_init =
                                    declaration.Type.cast(self.ast).to_spec_init(self)?;
                                Ok((ident, spec_init))
                            });
                        match parsed {
                            Ok((ident, spec_init)) => {
                                let constant = Pou::Constant(self.new_variable(
                                    ident,
                                    name.cast(self.ast).into(),
                                    VariableKind::Var,
                                    Qualifier::CONSTANT,
                                    false,
                                    spec_init.spec,
                                    spec_init.init,
                                    declaration.into(),
                                    scope_id,
                                ));
                                self.register_scope(
                                    ScopeKind::Pou(constant),
                                    vec![],
                                    scope_id,
                                    previous_scope,
                                );
                                constants.push(constant);
                            }
                            Err(err) => self.errors.push(err),
                        }
                        self.current_scope = previous_scope;
                    }
                }
            }
        }
        constants
    }
}
