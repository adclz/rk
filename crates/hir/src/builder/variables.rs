#![allow(unused)]
use std::ops::Deref;

use ast::generated::{ExternalVarKind, GlobalVarKind};
use auto_lsp::core::ast::AstNode;
use auto_lsp::lsp_types::{self, Position};
use auto_lsp::tree_sitter::{self, Point};
use auto_lsp::{
    anyhow,
    default::db::{BaseDatabase, file::File},
};
use salsa::Accumulator;

use crate::builder::expression::ParseDirectVariable;
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::{Parse, ParseSpec, ParseSpecInit, ParseVarSection, SpecInitResult};
use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::e0_syntax::SyntaxError;
use crate::hir_def::config::AccessDirection;
use crate::hir_def::expressions::spec::{ElementarySpec, Spec, SpecKind};
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::pous::variable::{DirectVariable, LocatedVariable, VariableDecl, VariableKind};
use crate::hir_def::program::ProgAccessDecl;
use crate::{AstId, HirNodeInfo};
use ide_diagnostic::IdeDiagnostic;

impl<'db> ParseVarSection<'db> for ast::generated::InputDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_InputVar::ERRVariableWithNoSpec(child) => {
                    sema.errors
                        .push(SyntaxError::MissingVarType(child.get_span()).to_diagnostic(sema.db));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_InputVar::InputVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::InputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec_init(sema);
                                let Some(result) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Input,
                                    false,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::InputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec(sema);
                                let Some(spec) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Input,
                                    false,
                                    spec,
                                    None,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::InputVarKind::EdgeDecl(edge_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = edge_decl.to_spec_init(sema);
                                let Some(result) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Input,
                                    false,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::InputVarKind::VariadicDecl(variadic_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = variadic_decl.Type.cast(sema.ast).to_spec(sema);
                                let Some(spec) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Input,
                                    true,
                                    spec,
                                    None,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::FbInputDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_FbInputVar::ERRVariableWithNoSpec(child) => {
                    sema.errors
                        .push(SyntaxError::MissingVarType(child.get_span()).to_diagnostic(sema.db));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_FbInputVar::FbInputVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::FbInputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec_init(sema);
                                let Some(result) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Input,
                                    false,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::FbInputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec(sema);
                                let Some(spec) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Input,
                                    false,
                                    spec,
                                    None,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::FbInputVarKind::EdgeDecl(edge_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = edge_decl.to_spec_init(sema);
                                let Some(result) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Input,
                                    false,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::OutputDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_OutputVar::ERRVariableWithNoSpec(child) => {
                    sema.errors
                        .push(SyntaxError::MissingVarType(child.get_span()).to_diagnostic(sema.db));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_OutputVar::OutputVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::OutputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec_init(sema);
                                let Some(result) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Output,
                                    false,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::OutputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec(sema);
                                let Some(spec) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Output,
                                    false,
                                    spec,
                                    None,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::FbOutputDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_FbOutputVar::ERRVariableWithNoSpec(child) => {
                    sema.errors
                        .push(SyntaxError::MissingVarType(child.get_span()).to_diagnostic(sema.db));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_FbOutputVar::FbOutputVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::FbOutputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec_init(sema);
                                let Some(result) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Output,
                                    false,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::FbOutputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec(sema);
                                let Some(spec) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Output,
                                    false,
                                    spec,
                                    None,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::TempVarDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_TempVar::ERRVariableWithNoSpec(child) => {
                    sema.errors
                        .push(SyntaxError::MissingVarType(child.get_span()).to_diagnostic(sema.db));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_TempVar::TempVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::TempVarKind::VarDecl(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec_init(sema);
                                let Some(result) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Temp,
                                    false,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::TempVarKind::RefSpec(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec_init(sema);
                                let Some(result) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::Temp,
                                    false,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::InOutDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_InOutVar::ERRVariableWithNoSpec(child) => {
                    sema.errors
                        .push(SyntaxError::MissingVarType(child.get_span()).to_diagnostic(sema.db));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_InOutVar::InOutVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::InOutVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec(sema);
                                let Some(spec) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::InOut,
                                    false,
                                    spec,
                                    None,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::InOutVarKind::VarDecl(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let r =
                                    Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                                let Some(var_name) = sema.try_parse(r) else {
                                    continue;
                                };
                                let r = var_decl.to_spec_init(sema);
                                let Some(result) = sema.try_parse(r) else {
                                    continue;
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    variable.cast(sema.ast).into(),
                                    VariableKind::InOut,
                                    false,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::ExternalVarDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_ExternalDecl::ExternalDecl(child) => {
                    match child.Type.cast(sema.ast) {
                        ExternalVarKind::VarDecl(var_decl) => {
                            let r = Ident::from_node(sema.db, sema.file, child.name.cast(sema.ast));
                            let Some(var_name) = sema.try_parse(r) else {
                                continue;
                            };
                            let r = var_decl.to_spec_init(sema);
                            let Some(result) = sema.try_parse(r) else {
                                continue;
                            };
                            section.push(VariableDecl::new(
                                sema.db,
                                var_name,
                                child.name.cast(sema.ast).into(),
                                VariableKind::External,
                                false,
                                result.spec,
                                result.init,
                                child.into(),
                                sema.current_scope,
                            ));
                        }
                        ExternalVarKind::ArrayConformand(var_decl) => {
                            let r = Ident::from_node(sema.db, sema.file, child.name.cast(sema.ast));
                            let Some(var_name) = sema.try_parse(r) else {
                                continue;
                            };
                            let r = var_decl.to_spec(sema);
                            let Some(spec) = sema.try_parse(r) else {
                                continue;
                            };
                            section.push(VariableDecl::new(
                                sema.db,
                                var_name,
                                child.name.cast(sema.ast).into(),
                                VariableKind::External,
                                false,
                                spec,
                                None,
                                child.into(),
                                sema.current_scope,
                            ));
                        }
                    }
                }
                ast::generated::ERRVariableWithNoSpec_ExternalDecl::ERRVariableWithNoSpec(
                    child,
                ) => {
                    sema.errors
                        .push(SyntaxError::MissingVarType(child.get_span()).to_diagnostic(sema.db));
                    continue;
                }
            }
        }
    }
}

pub trait ParseLocatedVar<'db> {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<LocatedVariable<'db>>);
}

impl<'db> ParseLocatedVar<'db> for ast::generated::LocVarDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<LocatedVariable<'db>>) {
        for variable in self.children.iter() {
            let variable = variable.cast(sema.ast);
            let var_name = if let Some(name) = &variable.variable_name {
                let r = Ident::from_node(sema.db, sema.file, name.cast(sema.ast));
                let Some(name) = sema.try_parse(r) else {
                    continue;
                };
                Some(name)
            } else {
                None
            };

            let r = variable
                .located_at
                .cast(sema.ast)
                .children
                .cast(sema.ast)
                .to_direct_variable(sema);
            let Some(located_at) = sema.try_parse(r) else {
                continue;
            };

            let r = variable.spec_init.cast(sema.ast).to_spec_init(sema);
            let Some(spec_init) = sema.try_parse(r) else {
                continue;
            };

            section.push(LocatedVariable::new(
                sema.db,
                var_name,
                located_at,
                spec_init.spec,
                spec_init.init,
            ));
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::VarDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::ERRVariableWithNoSpec(
                    child,
                ) => {
                    sema.errors
                        .push(SyntaxError::MissingVarType(child.get_span()).to_diagnostic(sema.db));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::VarDeclInitList(
                    var_decl,
                ) => {
                    for variable in var_decl.variables.cast(sema.ast).children.iter() {
                        let r = Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                        let Some(var_name) = sema.try_parse(r) else {
                            continue;
                        };
                        let r = var_decl.Type.cast(sema.ast).to_spec_init(sema);
                        let Some(result) = sema.try_parse(r) else {
                            continue;
                        };
                        section.push(VariableDecl::new(
                            sema.db,
                            var_name,
                            variable.cast(sema.ast).into(),
                            VariableKind::Var,
                            false,
                            result.spec,
                            result.init,
                            var_decl.into(),
                            sema.current_scope,
                        ));
                    }
                }
            }
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::RetainVarDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::ERRVariableWithNoSpec(
                    child,
                ) => {
                    sema.errors
                        .push(SyntaxError::MissingVarType(child.get_span()).to_diagnostic(sema.db));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::VarDeclInitList(
                    var_decl,
                ) => {
                    for variable in var_decl.variables.cast(sema.ast).children.iter() {
                        let r = Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                        let Some(var_name) = sema.try_parse(r) else {
                            continue;
                        };
                        let r = var_decl.Type.cast(sema.ast).to_spec_init(sema);
                        let Some(result) = sema.try_parse(r) else {
                            continue;
                        };
                        section.push(VariableDecl::new(
                            sema.db,
                            var_name,
                            variable.cast(sema.ast).into(),
                            VariableKind::Var,
                            false,
                            result.spec,
                            result.init,
                            var_decl.into(),
                            sema.current_scope,
                        ));
                    }
                }
            }
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::NoRetainVarDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::ERRVariableWithNoSpec(
                    err,
                ) => {
                    sema.errors
                        .push(SyntaxError::MissingVarType(err.get_span()).to_diagnostic(sema.db));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::VarDeclInitList(
                    var_decl,
                ) => {
                    for variable in var_decl.variables.cast(sema.ast).children.iter() {
                        let r = Ident::from_node(sema.db, sema.file, variable.cast(sema.ast));
                        let Some(var_name) = sema.try_parse(r) else {
                            continue;
                        };
                        let r = var_decl.Type.cast(sema.ast).to_spec_init(sema);
                        let Some(result) = sema.try_parse(r) else {
                            continue;
                        };
                        section.push(VariableDecl::new(
                            sema.db,
                            var_name,
                            variable.cast(sema.ast).into(),
                            VariableKind::Var,
                            false,
                            result.spec,
                            result.init,
                            var_decl.into(),
                            sema.current_scope,
                        ));
                    }
                }
            }
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::LocPartlyVarDecl {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            let r = Ident::from_node(
                sema.db,
                sema.file,
                child.cast(sema.ast).variable_name.cast(sema.ast),
            );
            let Some(var_name) = sema.try_parse(r) else {
                continue;
            };
            let r = child.cast(sema.ast).to_spec_init(sema);
            let Some(result) = sema.try_parse(r) else {
                continue;
            };
            section.push(VariableDecl::new(
                sema.db,
                var_name,
                child.cast(sema.ast).variable_name.cast(sema.ast).into(),
                VariableKind::Var,
                false,
                result.spec,
                result.init,
                child.cast(sema.ast).into(),
                sema.current_scope,
            ));
        }
    }
}

pub trait ParseProgDecl<'db> {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<ProgAccessDecl<'db>>);
}

impl<'db> ParseProgDecl<'db> for ast::generated::ProgAccessDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<ProgAccessDecl<'db>>) {
        for child in self.children.iter() {
            let decl = child.cast(sema.ast);
            let r = decl.access.cast(sema.ast).to_spec(sema);
            let Some(spec) = sema.try_parse(r) else {
                continue;
            };

            let r = Ident::from_node(sema.db, sema.file, decl.name.cast(sema.ast));
            let Some(name) = sema.try_parse(r) else {
                continue;
            };

            let direct_variable = match &decl.children {
                Some(v) => {
                    let r = v.cast(sema.ast).to_direct_variable(sema);
                    let Some(dv) = sema.try_parse(r) else {
                        continue;
                    };
                    Some(dv)
                }
                _ => None,
            };

            let r = decl.variable.cast(sema.ast).parse(sema);
            let Some(variable) = sema.try_parse(r) else {
                continue;
            };

            let direction = match &decl.direction {
                Some(direction) => match direction.cast(sema.ast).children.cast(sema.ast) {
                    ast::generated::ReadOnly_ReadWrite::ReadOnly(_) => {
                        Some(AccessDirection::ReadOnly)
                    }
                    ast::generated::ReadOnly_ReadWrite::ReadWrite(_) => {
                        Some(AccessDirection::ReadWrite)
                    }
                },
                None => None,
            };

            section.push(ProgAccessDecl {
                spec,
                name,
                variable,
                direct_variable,
                direction,
            })
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::GlobalVarDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast).Type.cast(sema.ast) {
                GlobalVarKind::NamespaceAccess(var_decl) => {
                    let r = Ident::from_node(
                        sema.db,
                        sema.file,
                        child.cast(sema.ast).spec.cast(sema.ast),
                    );
                    let Some(name) = sema.try_parse(r) else {
                        continue;
                    };
                    let r = var_decl.to_spec_init(sema);
                    let Some(result) = sema.try_parse(r) else {
                        continue;
                    };
                    section.push(VariableDecl::new(
                        sema.db,
                        name,
                        child.cast(sema.ast).spec.cast(sema.ast).into(),
                        VariableKind::Global,
                        false,
                        result.spec,
                        result.init,
                        child.cast(sema.ast).into(),
                        sema.current_scope,
                    ))
                }
                GlobalVarKind::LocVarSpecInit(var_decl) => {
                    let r = Ident::from_node(
                        sema.db,
                        sema.file,
                        child.cast(sema.ast).spec.cast(sema.ast),
                    );
                    let Some(name) = sema.try_parse(r) else {
                        continue;
                    };
                    let r = var_decl.to_spec_init(sema);
                    let Some(result) = sema.try_parse(r) else {
                        continue;
                    };
                    section.push(VariableDecl::new(
                        sema.db,
                        name,
                        child.cast(sema.ast).spec.cast(sema.ast).into(),
                        VariableKind::Global,
                        false,
                        result.spec,
                        result.init,
                        child.cast(sema.ast).into(),
                        sema.current_scope,
                    ))
                }
            }
        }
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::EdgeDecl {
    fn to_spec_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>, IdeDiagnostic> {
        let spec = match self.edge.cast(sema.ast) {
            ast::generated::ERRInvalidEdgeQualifier_FEDGE_REDGE::ERRInvalidEdgeQualifier(err) => {
                sema.errors.push(
                    SyntaxError::IncompleteEdgeQualifier(err.get_span()).to_diagnostic(sema.db),
                );
                Spec::new(
                    sema.db,
                    SpecKind::Simple(ElementarySpec::Bool),
                    self.into(),
                    sema.current_scope,
                )
            }
            ast::generated::ERRInvalidEdgeQualifier_FEDGE_REDGE::Token_F_EDGE(fedge) => Spec::new(
                sema.db,
                SpecKind::Simple(ElementarySpec::FEDGEBool),
                self.into(),
                sema.current_scope,
            ),
            ast::generated::ERRInvalidEdgeQualifier_FEDGE_REDGE::Token_R_EDGE(redge) => Spec::new(
                sema.db,
                SpecKind::Simple(ElementarySpec::REDGEBool),
                self.into(),
                sema.current_scope,
            ),
        };

        Ok(SpecInitResult::new(spec, None))
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::LocPartlyVar {
    fn to_spec_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>, IdeDiagnostic> {
        let spec = self
            .spec
            .cast(sema.ast)
            .children
            .cast(sema.ast)
            .to_spec(sema)?;
        Ok(SpecInitResult::new(spec, None))
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::VarDecl {
    fn to_spec_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>, IdeDiagnostic> {
        type Spec = ast::generated::ArrayTypeSpec_SimpleTypeSpec_StructTypeSpec;

        let spec = match self.spec.cast(sema.ast) {
            Spec::ArrayTypeSpec(a) => a.to_spec(sema),
            Spec::SimpleTypeSpec(a) => a.to_spec(sema),
            Spec::StructTypeSpec(a) => a.to_spec(sema),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

        let init = match self.init.as_ref().map(|i| i.cast(sema.ast)) {
            Some(Init::ArrayTypeInit(a)) => {
                let r = a.parse(sema);
                sema.try_parse(r)
            }
            Some(Init::SimpleTypeInit(a)) => {
                let r = a.parse(sema);
                sema.try_parse(r)
            }
            Some(Init::StructTypeInit(a)) => {
                let r = a.parse(sema);
                sema.try_parse(r)
            }
            None => None,
        };

        if let Some(init) = &self.init {
            sema.errors.push(
                SyntaxError::UnexpectedVarInit(init.cast(sema.ast).get_span())
                    .to_diagnostic(sema.db),
            );
        }

        match spec {
            Ok(spec) => Ok(SpecInitResult::new(spec, None)),
            Err(err) => Err(err),
        }
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::VarDeclInit {
    fn to_spec_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>, IdeDiagnostic> {
        type Spec = ast::generated::ArrayTypeSpec_RefTypeSpec_SimpleTypeSpec_StructTypeSpec;

        let spec = match self.spec.cast(sema.ast) {
            Spec::ArrayTypeSpec(a) => a.to_spec(sema),
            Spec::SimpleTypeSpec(a) => a.to_spec(sema),
            Spec::StructTypeSpec(a) => a.to_spec(sema),
            Spec::RefTypeSpec(target) => target.to_spec(sema),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;
        let init = match self.init.as_ref().map(|i| i.cast(sema.ast)) {
            Some(Init::ArrayTypeInit(a)) => {
                let r = a.parse(sema);
                sema.try_parse(r)
            }
            Some(Init::SimpleTypeInit(a)) => {
                let r = a.parse(sema);
                sema.try_parse(r)
            }
            Some(Init::StructTypeInit(a)) => {
                let r = a.parse(sema);
                sema.try_parse(r)
            }
            None => None,
        };

        match spec {
            Ok(spec) => Ok(SpecInitResult::new(spec, init)),
            Err(err) => Err(err),
        }
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::LocVarSpecInit {
    fn to_spec_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>, IdeDiagnostic> {
        type Spec = ast::generated::ArrayTypeSpec_SimpleTypeSpec_StructTypeSpec;

        let spec = match self.spec.cast(sema.ast) {
            Spec::ArrayTypeSpec(a) => a.to_spec(sema),
            Spec::SimpleTypeSpec(a) => a.to_spec(sema),
            Spec::StructTypeSpec(a) => a.to_spec(sema),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;
        let init = match self.init.as_ref().map(|i| i.cast(sema.ast)) {
            Some(Init::ArrayTypeInit(a)) => {
                let r = a.parse(sema);
                sema.try_parse(r)
            }
            Some(Init::SimpleTypeInit(a)) => {
                let r = a.parse(sema);
                sema.try_parse(r)
            }
            Some(Init::StructTypeInit(a)) => {
                let r = a.parse(sema);
                sema.try_parse(r)
            }
            None => None,
        };

        match spec {
            Ok(spec) => Ok(SpecInitResult::new(spec, init)),
            Err(err) => Err(err),
        }
    }
}
