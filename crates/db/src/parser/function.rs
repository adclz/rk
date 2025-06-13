use std::ops::Deref;

use auto_lsp::default::db::{BaseDatabase, File};
use crate::hir::function::Variable;
use crate::parser::Parse;
use crate::hir;

impl<'db> Parse<'db> for ast::generated::FuncDecl {
    type Output = hir::function::Function<'db>;

    fn parse(&self, _db: &'db dyn BaseDatabase, _file: File) -> Self::Output {
        //hir::function::Function::new(db)
        todo!()
    }
}

trait ParseVariable<'db> {
    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> Vec<Variable<'db>>;
}

impl<'db> ParseVariable<'db> for ast::generated::FuncDecl {
    fn parse(&self, _db: &'db dyn BaseDatabase, _file: File) -> Vec<Variable<'db>> {
        type VariableDecl = ast::generated::FuncVarDecls_IoVarDecls_TempVarDecls;
        self.variables.iter().for_each(|variable| {
            match variable.deref() {
                VariableDecl::FuncVarDecls(variable) => {
                    match variable.children.deref() {
                        ast::generated::ExternalVarDecls_VarDecls::VarDecls(variable) => {
                            variable.children.iter().for_each(|variable| {
                                
                            });
                        }
                        ast::generated::ExternalVarDecls_VarDecls::ExternalVarDecls(variable) => {
                            variable.children.iter().for_each(|variable| {
                                
                            });
                        }
                    }
                }
                VariableDecl::IoVarDecls(variable) => {
                    match variable.children.deref() {
                        ast::generated::InOutDecls_InputDecls_OutputDecls::InputDecls(variable) => {
                            variable.children.iter().for_each(|variable| {

                            });
                        }
                        ast::generated::InOutDecls_InputDecls_OutputDecls::InOutDecls(variable) => {
                            variable.children.iter().for_each(|variable| {
                                
                            });
                        }
                        ast::generated::InOutDecls_InputDecls_OutputDecls::OutputDecls(variable) => {
                            variable.children.iter().for_each(|variable| {
                                
                            });
                        }
                    }
                }
                VariableDecl::TempVarDecls(variable) => {
                    variable.children.iter().for_each(|variable| {
                        match variable.deref() {
                            ast::generated::InterfaceVarDecl_RefVarDecl_VarDecl::InterfaceVarDecl(variable) => {
                                variable.children.iter().for_each(|variable| {
                                
                                });
                            }
                            ast::generated::InterfaceVarDecl_RefVarDecl_VarDecl::RefVarDecl(variable) => {
                                variable.children.iter().for_each(|variable| {
                                
                                });
                            }
                            ast::generated::InterfaceVarDecl_RefVarDecl_VarDecl::VarDecl(variable) => {
                                variable.children.iter().for_each(|variable| {
                                
                                });
                            }
                        }
                    });
                }
            }
        });
        
        vec![]
    }
}