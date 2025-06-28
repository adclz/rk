#![allow(unused)]
use std::ops::Deref;

use ast::generated::{ExternalVarKind, GlobalVarKind};
use auto_lsp::{
    anyhow,
    default::db::{BaseDatabase, File},
};
use auto_lsp::core::ast::AstNode;

use crate::{hir::variable::Variable, ident::Ident};

pub trait ParseVarSection<'db> {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()>;
}

macro_rules! parse_multi_variable_sections {
    ($($section: ident, [$( $section_kind: path ),*]), *) => {
    $(impl<'db> ParseVarSection<'db> for ast::generated::$section {
        fn parse(
        &self,
            db: &'db dyn BaseDatabase,
            file: File,
            section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.Type.deref() {
                $($section_kind(var_decl) => {
                    for variable in child.variables.children.iter() {
                        let name = Ident::from_node(db, file, variable.deref())?;
                        section.push(Variable::new(db, name, *variable.get_range(), *variable.get_range()));
                    }
                }),*
            }
        }
        Ok(())
    }
    })*
    };
}

parse_multi_variable_sections! {
    InputDecls, [
        ast::generated::InputVarKind::VarDeclInit,
        ast::generated::InputVarKind::ArrayConformand,
        ast::generated::InputVarKind::EdgeDecl
    ],
    FbInputDecls, [
        ast::generated::FbInputVarKind::VarDeclInit,
        ast::generated::FbInputVarKind::ArrayConformand,
        ast::generated::FbInputVarKind::EdgeDecl
    ],
    OutputDecls, [
        ast::generated::OutputVarKind::VarDeclInit,
        ast::generated::OutputVarKind::ArrayConformand
    ],
    FbOutputDecls, [
        ast::generated::FbOutputVarKind::VarDeclInit,
        ast::generated::FbOutputVarKind::ArrayConformand
    ],
    TempVarDecls, [
        ast::generated::TempVarKind::VarDecl,
        ast::generated::TempVarKind::RefSpec
    ],
    InOutDecls, [
        ast::generated::InOutVarKind::VarDecl,
        ast::generated::InOutVarKind::ArrayConformand,
        ast::generated::InOutVarKind::FbDeclNoInit
    ],
    VarDecls, [
        ast::generated::VarDecl::StrTypeSpecInit,
        ast::generated::VarDecl::ArrayTypeSpecInit,
        ast::generated::VarDecl::NamespaceQualifierTarget,
        ast::generated::VarDecl::SimpleTypeSpecInit,
        ast::generated::VarDecl::StructTypeSpecInit
    ],
    RetainVarDecls, [
        ast::generated::VarDecl::StrTypeSpecInit,
        ast::generated::VarDecl::ArrayTypeSpecInit,
        ast::generated::VarDecl::NamespaceQualifierTarget,
        ast::generated::VarDecl::SimpleTypeSpecInit,
        ast::generated::VarDecl::StructTypeSpecInit
    ],
    NoRetainVarDecls, [
        ast::generated::VarDecl::StrTypeSpecInit,
        ast::generated::VarDecl::ArrayTypeSpecInit,
        ast::generated::VarDecl::NamespaceQualifierTarget,
        ast::generated::VarDecl::SimpleTypeSpecInit,
        ast::generated::VarDecl::StructTypeSpecInit
    ]
}

impl<'db> ParseVarSection<'db> for ast::generated::ExternalVarDecls {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.Type.deref() {
                ExternalVarKind::VarDecl(var_decl) => {
                    let name = Ident::from_node(db, file, child.name.deref())?;
                    section.push(Variable::new(db, name, *child.get_range(), *child.name.get_range()))
                }
                ExternalVarKind::ArrayConformand(var_decl) => {
                    let name = Ident::from_node(db, file, child.name.deref())?;
                    section.push(Variable::new(db, name, *child.get_range(), *child.name.get_range()))
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::LocPartlyVarDecl {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            let name = Ident::from_node(db, file, child.variable_name.deref())?;
            section.push(Variable::new(db, name, *child.get_range(), *child.variable_name.get_range()))
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::GlobalVarDecls {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.Type.deref() {
                GlobalVarKind::NamespaceQualifierTarget(var_decl) => {
                    let name = Ident::from_node(db, file, child.spec.deref())?;
                    section.push(Variable::new(db, name, *child.get_range(), *child.spec.get_range()))
                }
                GlobalVarKind::LocVarSpecInit(var_decl) => {
                    let name = Ident::from_node(db, file, child.spec.deref())?;
                    section.push(Variable::new(db, name, *child.get_range(), *child.spec.get_range()))
                }
            }
        }
        Ok(())
    }
}

pub trait ParseVariable<'db> {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        name: Ident,
    ) -> anyhow::Result<Variable<'db>>;
}

impl<'db> ParseVariable<'db> for ast::generated::VarDecl {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        name: Ident,
    ) -> anyhow::Result<Variable<'db>> {
        match self {
            Self::StrTypeSpecInit(str_var_decl) => todo!(),
            Self::ArrayTypeSpecInit(array_spec) => todo!(),
            Self::NamespaceQualifierTarget(type_access) => todo!(),
            Self::SimpleTypeSpecInit(simple_type_spec_init) => todo!(),
            Self::StructTypeSpecInit(struct_type_spec_init) => todo!(),
        }
    }
}

impl<'db> ParseVariable<'db> for ast::generated::VarDeclInit {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        name: Ident,
    ) -> anyhow::Result<Variable<'db>> {
        match self {
            Self::NamespaceQualifierTarget(type_access) => todo!(),
            Self::ArrayTypeSpecInit(array) => todo!(),
            Self::InterfaceSpecInit(_) => todo!(),
            Self::RefSpecInit(_) => todo!(),
            Self::SimpleTypeSpecInit(_) => todo!(),
            Self::StrTypeSpecInit(_) => todo!(),
            Self::StructTypeSpecInit(_) => todo!(),
        }
    }
}

impl<'db> ParseVariable<'db> for ast::generated::StrTypeSpecInit {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        name: Ident,
    ) -> anyhow::Result<Variable<'db>> {
        todo!()
    }
}
