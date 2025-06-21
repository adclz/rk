use std::ops::Deref;

use ast::generated::{ExternalVarKind, GlobalVarKind};
use auto_lsp::{
    anyhow,
    default::db::{BaseDatabase, File},
};

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
                        section.push(Variable::from(db, name));
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
        ast::generated::InputVarKind::ArrayConformDecl,
        ast::generated::InputVarKind::EdgeDecl
    ],
    FbInputDecls, [
        ast::generated::FbInputVarKind::VarDeclInit,
        ast::generated::FbInputVarKind::ArrayConformDecl,
        ast::generated::FbInputVarKind::EdgeDecl
    ],
    OutputDecls, [
        ast::generated::OutputVarKind::VarDeclInit,
        ast::generated::OutputVarKind::ArrayConformDecl
    ],
    FbOutputDecls, [
        ast::generated::FbOutputVarKind::VarDeclInit,
        ast::generated::FbOutputVarKind::ArrayConformDecl
    ],
    TempVarDecls, [
        ast::generated::TempVarKind::VarDecl,
        ast::generated::TempVarKind::RefSpec
    ],
    InOutDecls, [
        ast::generated::InOutVarKind::VarDecl,
        ast::generated::InOutVarKind::ArrayConformDecl,
        ast::generated::InOutVarKind::FbDeclNoInit
    ],
    VarDecls, [
        ast::generated::VarDecl::ArraySpec,
        ast::generated::VarDecl::StrVarDecl,
        ast::generated::VarDecl::TypeAccess
    ],
    RetainVarDecls, [
        ast::generated::VarDecl::ArraySpec,
        ast::generated::VarDecl::StrVarDecl,
        ast::generated::VarDecl::TypeAccess
    ],
    NoRetainVarDecls, [
        ast::generated::VarDecl::ArraySpec,
        ast::generated::VarDecl::StrVarDecl,
        ast::generated::VarDecl::TypeAccess
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
                    section.push(Variable::from(db, name))
                }
                ExternalVarKind::ArrayConformDecl(var_decl) => {
                    let name = Ident::from_node(db, file, child.name.deref())?;
                    section.push(Variable::from(db, name))
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
            section.push(Variable::from(db, name))
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
                GlobalVarKind::TypeAccess(var_decl) => {
                    let name = Ident::from_node(db, file, child.spec.deref())?;
                    section.push(Variable::from(db, name))
                }
                GlobalVarKind::LocVarSpecInit(var_decl) => {
                    let name = Ident::from_node(db, file, child.spec.deref())?;
                    section.push(Variable::from(db, name))
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
            Self::StrVarDecl(str_var_decl) => match str_var_decl.children.deref() {
                ast::generated::DByteStrSpec_SByteStrSpec::DByteStrSpec(str) => {
                    todo!()
                }
                ast::generated::DByteStrSpec_SByteStrSpec::SByteStrSpec(w_str) => {
                    todo!()
                }
            },
            Self::ArraySpec(array_spec) => todo!(),
            Self::TypeAccess(type_access) => todo!(),
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
        match self.deref() {
            Self::ArraySpecInit(array) => todo!(),
            Self::InterfaceSpecInit(_) => todo!(),
            Self::RefSpecInit(_) => todo!(),
            Self::SimpleSpecInit(_) => todo!(),
            Self::StrVarDecl(_) => todo!(),
            Self::StructSpecInit(_) => todo!(),
        }
    }
}

impl<'db> ParseVariable<'db> for ast::generated::StrVarDecl {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        name: Ident,
    ) -> anyhow::Result<Variable<'db>> {
        match self.children.deref() {
            ast::generated::DByteStrSpec_SByteStrSpec::DByteStrSpec(_) => todo!(),
            ast::generated::DByteStrSpec_SByteStrSpec::SByteStrSpec(_) => todo!(),
        }
    }
}
