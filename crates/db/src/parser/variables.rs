#![allow(unused)]
use std::ops::Deref;

use ast::generated::{ExternalVarKind, GlobalVarKind};
use auto_lsp::core::ast::AstNode;
use auto_lsp::{
    anyhow,
    default::db::{BaseDatabase, file::File},
};

use crate::parser::{ParseInit, ParseSpec, ParseSpecInit, ParseVarSection, SpecInitResult};
use crate::{
    hir::variable::{Spec, Variable, VariableKind},
    ident::Ident,
};

macro_rules! parse_multi_variable_sections {
    ($($section: ident, $kind: ident, [$( $section_kind: path ),*]), *) => {
    $(impl<'db> ParseVarSection<'db> for ast::generated::$section {
        fn parse(
            &'db self,
            db: &'db dyn BaseDatabase,
            file: File,
            section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.Type.deref() {
                $($section_kind(var_decl) => {
                    for variable in child.variables.children.iter() {
                        let name = Ident::from_node(db, file, variable.deref())?;
                        let result = var_decl.to_spec_init(db, file)?;
                        section.push(
                            Variable::new(db,
                                name,
                                variable.get_span(),
                                variable.get_span(),
                                VariableKind::$kind,
                                result.spec,
                                result.init,
                            ));
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
    InputDecls, Input, [
        ast::generated::InputVarKind::VarDeclInit,
        ast::generated::InputVarKind::ArrayConformand,
        ast::generated::InputVarKind::EdgeDecl
    ],
    FbInputDecls, Input, [
        ast::generated::FbInputVarKind::VarDeclInit,
        ast::generated::FbInputVarKind::ArrayConformand,
        ast::generated::FbInputVarKind::EdgeDecl
    ],
    OutputDecls, Output, [
        ast::generated::OutputVarKind::VarDeclInit,
        ast::generated::OutputVarKind::ArrayConformand
    ],
    FbOutputDecls, Output, [
        ast::generated::FbOutputVarKind::VarDeclInit,
        ast::generated::FbOutputVarKind::ArrayConformand
    ],
    TempVarDecls, Temp, [
        ast::generated::TempVarKind::VarDecl,
        ast::generated::TempVarKind::RefSpec
    ],
    InOutDecls, InOut, [
        ast::generated::InOutVarKind::VarDecl,
        ast::generated::InOutVarKind::ArrayConformand,
        ast::generated::InOutVarKind::FbDeclNoInit
    ]
}

impl<'db> ParseVarSection<'db> for ast::generated::ExternalVarDecls {
    fn parse(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.Type.deref() {
                ExternalVarKind::VarDecl(var_decl) => {
                    let name = Ident::from_node(db, file, child.name.deref())?;
                    let result = var_decl.to_spec_init(db, file)?;
                    section.push(Variable::new(
                        db,
                        name,
                        child.get_span(),
                        child.name.get_span(),
                        VariableKind::External,
                        result.spec,
                        result.init,
                    ))
                }
                ExternalVarKind::ArrayConformand(var_decl) => {
                    let name = Ident::from_node(db, file, child.name.deref())?;
                    let result = var_decl.to_spec_init(db, file)?;
                    section.push(Variable::new(
                        db,
                        name,
                        child.get_span(),
                        child.name.get_span(),
                        VariableKind::External,
                        result.spec,
                        result.init,
                    ))
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::VarDecls {
    fn parse(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            for variable in child.variables.children.iter() {
                let name = Ident::from_node(db, file, variable.deref())?;
                let result = child.Type.to_spec_init(db, file)?;
                section.push(Variable::new(
                    db,
                    name,
                    child.get_span(),
                    variable.get_span(),
                    VariableKind::Local,
                    result.spec,
                    result.init,
                ))
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::RetainVarDecls {
    fn parse(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            for variable in child.variables.children.iter() {
                let name = Ident::from_node(db, file, variable.deref())?;
                let result = child.Type.to_spec_init(db, file)?;
                section.push(Variable::new(
                    db,
                    name,
                    child.get_span(),
                    variable.get_span(),
                    VariableKind::Retain,
                    result.spec,
                    result.init,
                ))
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::NoRetainVarDecls {
    fn parse(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            for variable in child.variables.children.iter() {
                let name = Ident::from_node(db, file, variable.deref())?;
                let result = child.Type.to_spec_init(db, file)?;
                section.push(Variable::new(
                    db,
                    name,
                    child.get_span(),
                    variable.get_span(),
                    VariableKind::NoRetain,
                    result.spec,
                    result.init,
                ))
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::LocPartlyVarDecl {
    fn parse(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            let name = Ident::from_node(db, file, child.variable_name.deref())?;
            let result = child.to_spec_init(db, file)?;
            section.push(Variable::new(
                db,
                name,
                child.get_span(),
                child.variable_name.get_span(),
                VariableKind::LocPartly,
                result.spec,
                result.init,
            ))
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::GlobalVarDecls {
    fn parse(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.Type.deref() {
                GlobalVarKind::FqName(var_decl) => {
                    let name = Ident::from_node(db, file, child.spec.deref())?;
                    let result = var_decl.to_spec_init(db, file)?;
                    section.push(Variable::new(
                        db,
                        name,
                        child.get_span(),
                        child.spec.get_span(),
                        VariableKind::Global,
                        result.spec,
                        result.init,
                    ))
                }
                GlobalVarKind::LocVarSpecInit(var_decl) => {
                    let name = Ident::from_node(db, file, child.spec.deref())?;
                    let result = var_decl.to_spec_init(db, file)?;
                    section.push(Variable::new(
                        db,
                        name,
                        child.get_span(),
                        child.spec.get_span(),
                        VariableKind::Global,
                        result.spec,
                        result.init,
                    ))
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::EdgeDecl {
    fn to_spec_init(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<SpecInitResult> {
        todo!()
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::LocPartlyVar {
    fn to_spec_init(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<SpecInitResult> {
        todo!()
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::FbDeclNoInit {
    fn to_spec_init(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<SpecInitResult> {
        todo!()
    }
}

// todo: VarDecl should contain no init
impl<'db> ParseSpecInit<'db> for ast::generated::VarDecl {
    fn to_spec_init(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        type Spec = ast::generated::ArrayTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec;

        let spec = match self.spec.deref() {
            Spec::ArrayTypeSpec(a) => a.to_spec(db, file),
            Spec::SimpleTypeSpec(a) => a.to_spec(db, file),
            Spec::StrTypeSpec(a) => a.to_spec(db, file),
            Spec::StructTypeSpec(a) => a.to_spec(db, file),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

        let init = match self.init.as_deref() {
            Some(Init::ArrayTypeInit(a)) => Some(a.to_init(db, file)?),
            Some(Init::SimpleTypeInit(a)) => Some(a.to_init(db, file)?),
            Some(Init::StructTypeInit(a)) => Some(a.to_init(db, file)?),
            None => None,
        };

        Ok(SpecInitResult::new(spec?, init))
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::VarDeclInit {
    fn to_spec_init(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        type Spec = ast::generated::ArrayTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec;

        let spec = match self.spec.deref() {
            Spec::ArrayTypeSpec(a) => a.to_spec(db, file),
            Spec::SimpleTypeSpec(a) => a.to_spec(db, file),
            Spec::StrTypeSpec(a) => a.to_spec(db, file),
            Spec::StructTypeSpec(a) => a.to_spec(db, file),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;
        let init = match self.init.as_deref() {
            Some(Init::ArrayTypeInit(a)) => Some(a.to_init(db, file)?),
            Some(Init::SimpleTypeInit(a)) => Some(a.to_init(db, file)?),
            Some(Init::StructTypeInit(a)) => Some(a.to_init(db, file)?),
            None => None,
        };

        Ok(SpecInitResult::new(spec?, init))
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::LocVarSpecInit {
    fn to_spec_init(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        type Spec = ast::generated::ArrayTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec;

        let spec = match self.spec.deref() {
            Spec::ArrayTypeSpec(a) => a.to_spec(db, file),
            Spec::SimpleTypeSpec(a) => a.to_spec(db, file),
            Spec::StrTypeSpec(a) => a.to_spec(db, file),
            Spec::StructTypeSpec(a) => a.to_spec(db, file),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;
        let init = match self.init.as_deref() {
            Some(Init::ArrayTypeInit(a)) => Some(a.to_init(db, file)?),
            Some(Init::SimpleTypeInit(a)) => Some(a.to_init(db, file)?),
            Some(Init::StructTypeInit(a)) => Some(a.to_init(db, file)?),
            None => None,
        };

        Ok(SpecInitResult::new(spec?, init))
    }
}
