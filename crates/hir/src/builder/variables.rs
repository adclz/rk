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

use crate::builder::expression::ParseExpr;
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::{ParseInit, ParseSpec, ParseSpecInit, ParseVarSection, SpecInitResult};
use crate::check::errors::sem_errors::{AnalysisError, SyntaxError};
use crate::hir_def::expressions::spec::{ElementarySpec, Spec, SpecKind};
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::pous::variable::{VariableDecl, VariableKind};
use crate::to_proto::{AstId, ToProto};

impl<'db> ParseVarSection<'db> for ast::generated::InputDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast) {
                ast::generated::ERRVariableWithNoSpec_InputVar::ERRVariableWithNoSpec(child) => {
                    sema.errors
                        .push(AnalysisError::SyntaxError(SyntaxError::MissingVarType(
                            child.get_span(),
                        )));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_InputVar::InputVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::InputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let result = match var_decl.to_spec_init(sema) {
                                    Ok(result) => result,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Input,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::InputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let spec = match var_decl.to_spec(sema) {
                                    Ok(spec) => spec,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Input,
                                    spec,
                                    None,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::InputVarKind::EdgeDecl(edge_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let result = match edge_decl.to_spec_init(sema) {
                                    Ok(result) => result,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Input,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
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
                        .push(AnalysisError::SyntaxError(SyntaxError::MissingVarType(
                            child.get_span(),
                        )));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_FbInputVar::FbInputVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::FbInputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let result = match var_decl.to_spec_init(sema) {
                                    Ok(result) => result,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Input,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::FbInputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let spec = match var_decl.to_spec(sema) {
                                    Ok(spec) => spec,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Input,
                                    spec,
                                    None,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::FbInputVarKind::EdgeDecl(edge_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let result = match edge_decl.to_spec_init(sema) {
                                    Ok(result) => result,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Input,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
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
                        .push(AnalysisError::SyntaxError(SyntaxError::MissingVarType(
                            child.get_span(),
                        )));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_OutputVar::OutputVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::OutputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let result = match var_decl.to_spec_init(sema) {
                                    Ok(result) => result,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Output,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::OutputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let spec = match var_decl.to_spec(sema) {
                                    Ok(spec) => spec,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Output,
                                    spec,
                                    None,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
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
                        .push(AnalysisError::SyntaxError(SyntaxError::MissingVarType(
                            child.get_span(),
                        )));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_FbOutputVar::FbOutputVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::FbOutputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let result = match var_decl.to_spec_init(sema) {
                                    Ok(result) => result,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Output,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::FbOutputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let spec = match var_decl.to_spec(sema) {
                                    Ok(spec) => spec,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Output,
                                    spec,
                                    None,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
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
                        .push(AnalysisError::SyntaxError(SyntaxError::MissingVarType(
                            child.get_span(),
                        )));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_TempVar::TempVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::TempVarKind::VarDecl(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let result = match var_decl.to_spec_init(sema) {
                                    Ok(result) => result,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Temp,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::TempVarKind::RefSpec(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let result = match var_decl.to_spec_init(sema) {
                                    Ok(result) => result,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::Temp,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
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
                        .push(AnalysisError::SyntaxError(SyntaxError::MissingVarType(
                            child.get_span(),
                        )));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_InOutVar::InOutVar(child) => {
                    match child.Type.cast(sema.ast) {
                        ast::generated::InOutVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let spec = match var_decl.to_spec(sema) {
                                    Ok(spec) => spec,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::InOut,
                                    spec,
                                    None,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
                                    sema.current_scope,
                                ));
                            }
                        }
                        ast::generated::InOutVarKind::VarDecl(var_decl) => {
                            for variable in child.variables.cast(sema.ast).children.iter() {
                                let var_name = match Ident::from_node(
                                    sema.db,
                                    sema.file,
                                    variable.cast(sema.ast),
                                ) {
                                    Ok(name) => name,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                let result = match var_decl.to_spec_init(sema) {
                                    Ok(result) => result,
                                    Err(err) => {
                                        sema.errors.push(err);
                                        continue;
                                    }
                                };
                                section.push(VariableDecl::new(
                                    sema.db,
                                    var_name,
                                    VariableKind::InOut,
                                    result.spec,
                                    result.init,
                                    child.into(),
                                    variable.cast(sema.ast).into(),
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
                            let var_name = match Ident::from_node(
                                sema.db,
                                sema.file,
                                child.name.cast(sema.ast),
                            ) {
                                Ok(name) => name,
                                Err(err) => {
                                    sema.errors.push(err);
                                    continue;
                                }
                            };
                            let result = match var_decl.to_spec_init(sema) {
                                Ok(result) => result,
                                Err(err) => {
                                    sema.errors.push(err);
                                    continue;
                                }
                            };
                            section.push(VariableDecl::new(
                                sema.db,
                                var_name,
                                VariableKind::External,
                                result.spec,
                                result.init,
                                child.into(),
                                child.name.cast(sema.ast).into(),
                                sema.current_scope,
                            ));
                        }
                        ExternalVarKind::ArrayConformand(var_decl) => {
                            let var_name = match Ident::from_node(
                                sema.db,
                                sema.file,
                                child.name.cast(sema.ast),
                            ) {
                                Ok(name) => name,
                                Err(err) => {
                                    sema.errors.push(err);
                                    continue;
                                }
                            };
                            let spec = match var_decl.to_spec(sema) {
                                Ok(spec) => spec,
                                Err(err) => {
                                    sema.errors.push(err);
                                    continue;
                                }
                            };
                            section.push(VariableDecl::new(
                                sema.db,
                                var_name,
                                VariableKind::External,
                                spec,
                                None,
                                child.into(),
                                child.name.cast(sema.ast).into(),
                                sema.current_scope,
                            ));
                        }
                    }
                }
                ast::generated::ERRVariableWithNoSpec_ExternalDecl::ERRVariableWithNoSpec(
                    child,
                ) => {
                    sema.errors
                        .push(AnalysisError::SyntaxError(SyntaxError::MissingVarType(
                            child.get_span(),
                        )));
                    continue;
                }
            }
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
                        .push(AnalysisError::SyntaxError(SyntaxError::MissingVarType(
                            child.get_span(),
                        )));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::VarDeclInitList(
                    var_decl,
                ) => {
                    for variable in var_decl.variables.cast(sema.ast).children.iter() {
                        let var_name =
                            match Ident::from_node(sema.db, sema.file, variable.cast(sema.ast)) {
                                Ok(name) => name,
                                Err(err) => {
                                    sema.errors.push(err);
                                    continue;
                                }
                            };
                        let result = match var_decl.Type.cast(sema.ast).to_spec_init(sema) {
                            Ok(result) => result,
                            Err(err) => {
                                sema.errors.push(err);
                                continue;
                            }
                        };
                        section.push(VariableDecl::new(
                            sema.db,
                            var_name,
                            VariableKind::Var,
                            result.spec,
                            result.init,
                            var_decl.into(),
                            variable.cast(sema.ast).into(),
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
                        .push(AnalysisError::SyntaxError(SyntaxError::MissingVarType(
                            child.get_span(),
                        )));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::VarDeclInitList(
                    var_decl,
                ) => {
                    for variable in var_decl.variables.cast(sema.ast).children.iter() {
                        let var_name =
                            match Ident::from_node(sema.db, sema.file, variable.cast(sema.ast)) {
                                Ok(name) => name,
                                Err(err) => {
                                    sema.errors.push(err);
                                    continue;
                                }
                            };
                        let result = match var_decl.Type.cast(sema.ast).to_spec_init(sema) {
                            Ok(result) => result,
                            Err(err) => {
                                sema.errors.push(err);
                                continue;
                            }
                        };
                        section.push(VariableDecl::new(
                            sema.db,
                            var_name,
                            VariableKind::Var,
                            result.spec,
                            result.init,
                            var_decl.into(),
                            variable.cast(sema.ast).into(),
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
                        .push(AnalysisError::SyntaxError(SyntaxError::MissingVarType(
                            err.get_span(),
                        )));
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::VarDeclInitList(
                    var_decl,
                ) => {
                    for variable in var_decl.variables.cast(sema.ast).children.iter() {
                        let var_name =
                            match Ident::from_node(sema.db, sema.file, variable.cast(sema.ast)) {
                                Ok(name) => name,
                                Err(err) => {
                                    sema.errors.push(err);
                                    continue;
                                }
                            };
                        let result = match var_decl.Type.cast(sema.ast).to_spec_init(sema) {
                            Ok(result) => result,
                            Err(err) => {
                                sema.errors.push(err);
                                continue;
                            }
                        };
                        section.push(VariableDecl::new(
                            sema.db,
                            var_name,
                            VariableKind::Var,
                            result.spec,
                            result.init,
                            var_decl.into(),
                            variable.cast(sema.ast).into(),
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
            let var_name = match Ident::from_node(
                sema.db,
                sema.file,
                child.cast(sema.ast).variable_name.cast(sema.ast),
            ) {
                Ok(name) => name,
                Err(err) => {
                    sema.errors.push(err);
                    continue;
                }
            };
            let result = match child.cast(sema.ast).to_spec_init(sema) {
                Ok(result) => result,
                Err(err) => {
                    sema.errors.push(err);
                    continue;
                }
            };
            section.push(VariableDecl::new(
                sema.db,
                var_name,
                VariableKind::Var,
                result.spec,
                result.init,
                child.cast(sema.ast).into(),
                child.cast(sema.ast).variable_name.cast(sema.ast).into(),
                sema.current_scope,
            ));
        }
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::GlobalVarDecls {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>) {
        for child in self.children.iter() {
            match child.cast(sema.ast).Type.cast(sema.ast) {
                GlobalVarKind::NamespaceAccess(var_decl) => {
                    let name = match Ident::from_node(
                        sema.db,
                        sema.file,
                        child.cast(sema.ast).spec.cast(sema.ast),
                    ) {
                        Ok(name) => name,
                        Err(err) => {
                            sema.errors.push(err);
                            continue;
                        }
                    };
                    let result = match var_decl.to_spec_init(sema) {
                        Ok(result) => result,
                        Err(err) => {
                            sema.errors.push(err);
                            continue;
                        }
                    };
                    section.push(VariableDecl::new(
                        sema.db,
                        name,
                        VariableKind::Global,
                        result.spec,
                        result.init,
                        child.cast(sema.ast).into(),
                        child.cast(sema.ast).spec.cast(sema.ast).into(),
                        sema.current_scope,
                    ))
                }
                GlobalVarKind::LocVarSpecInit(var_decl) => {
                    let name = match Ident::from_node(
                        sema.db,
                        sema.file,
                        child.cast(sema.ast).spec.cast(sema.ast),
                    ) {
                        Ok(name) => name,
                        Err(err) => {
                            sema.errors.push(err);
                            continue;
                        }
                    };
                    let result = match var_decl.to_spec_init(sema) {
                        Ok(result) => result,
                        Err(err) => {
                            sema.errors.push(err);
                            continue;
                        }
                    };
                    section.push(VariableDecl::new(
                        sema.db,
                        name,
                        VariableKind::Global,
                        result.spec,
                        result.init,
                        child.cast(sema.ast).into(),
                        child.cast(sema.ast).spec.cast(sema.ast).into(),
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
    ) -> anyhow::Result<SpecInitResult<'db>, AnalysisError<'db>> {
        let spec = match self.edge.cast(sema.ast) {
            ast::generated::ERRInvalidEdgeQualifier_FEDGE_REDGE::ERRInvalidEdgeQualifier(err) => {
                sema.errors.push(AnalysisError::SyntaxError(
                    SyntaxError::IncompleteEdgeQualifier(err.get_span()),
                ));
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
    ) -> anyhow::Result<SpecInitResult<'db>, AnalysisError<'db>> {
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
    ) -> anyhow::Result<SpecInitResult<'db>, AnalysisError<'db>> {
        type Spec = ast::generated::ArrayTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec;

        let spec = match self.spec.cast(sema.ast) {
            Spec::ArrayTypeSpec(a) => a.to_spec(sema),
            Spec::SimpleTypeSpec(a) => a.to_spec(sema),
            Spec::StrTypeSpec(a) => a.to_spec(sema),
            Spec::StructTypeSpec(a) => a.to_spec(sema),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

        let init = match self.init.as_ref().map(|i| i.cast(sema.ast)) {
            Some(Init::ArrayTypeInit(a)) => match a.parse(sema) {
                Ok(init) => Some(init),
                Err(err) => {
                    sema.errors.push(err);
                    None
                }
            },
            Some(Init::SimpleTypeInit(a)) => match a.parse(sema) {
                Ok(init) => Some(init),
                Err(err) => {
                    sema.errors.push(err);
                    None
                }
            },
            Some(Init::StructTypeInit(a)) => match a.parse(sema) {
                Ok(init) => Some(init),
                Err(err) => {
                    sema.errors.push(err);
                    None
                }
            },
            None => None,
        };

        if let Some(init) = &init {
            sema.errors
                .push(AnalysisError::SyntaxError(SyntaxError::UnexpectedVarInit(
                    init.get_span(sema.db).clone(),
                )));
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
    ) -> anyhow::Result<SpecInitResult<'db>, AnalysisError<'db>> {
        type Spec =
            ast::generated::ArrayTypeSpec_RefTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec;

        let spec = match self.spec.cast(sema.ast) {
            Spec::ArrayTypeSpec(a) => a.to_spec(sema),
            Spec::SimpleTypeSpec(a) => a.to_spec(sema),
            Spec::StrTypeSpec(a) => a.to_spec(sema),
            Spec::StructTypeSpec(a) => a.to_spec(sema),
            Spec::RefTypeSpec(target) => target.to_spec(sema),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;
        let init = match self.init.as_ref().map(|i| i.cast(sema.ast)) {
            Some(Init::ArrayTypeInit(a)) => match a.parse(sema) {
                Ok(init) => Some(init),
                Err(err) => {
                    sema.errors.push(err);
                    None
                }
            },
            Some(Init::SimpleTypeInit(a)) => match a.parse(sema) {
                Ok(init) => Some(init),
                Err(err) => {
                    sema.errors.push(err);
                    None
                }
            },
            Some(Init::StructTypeInit(a)) => match a.parse(sema) {
                Ok(init) => Some(init),
                Err(err) => {
                    sema.errors.push(err);
                    None
                }
            },
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
    ) -> anyhow::Result<SpecInitResult<'db>, AnalysisError<'db>> {
        type Spec = ast::generated::ArrayTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec;

        let spec = match self.spec.cast(sema.ast) {
            Spec::ArrayTypeSpec(a) => a.to_spec(sema),
            Spec::SimpleTypeSpec(a) => a.to_spec(sema),
            Spec::StrTypeSpec(a) => a.to_spec(sema),
            Spec::StructTypeSpec(a) => a.to_spec(sema),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;
        let init = match self.init.as_ref().map(|i| i.cast(sema.ast)) {
            Some(Init::ArrayTypeInit(a)) => match a.parse(sema) {
                Ok(init) => Some(init),
                Err(err) => {
                    sema.errors.push(err);
                    None
                }
            },
            Some(Init::SimpleTypeInit(a)) => match a.parse(sema) {
                Ok(init) => Some(init),
                Err(err) => {
                    sema.errors.push(err);
                    None
                }
            },
            Some(Init::StructTypeInit(a)) => match a.parse(sema) {
                Ok(init) => Some(init),
                Err(err) => {
                    sema.errors.push(err);
                    None
                }
            },
            None => None,
        };

        match spec {
            Ok(spec) => Ok(SpecInitResult::new(spec, init)),
            Err(err) => Err(err),
        }
    }
}
