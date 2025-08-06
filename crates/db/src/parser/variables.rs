#![allow(unused)]
use std::ops::Deref;

use ast::generated::{ExternalVarKind, GlobalVarKind};
use auto_lsp::core::ast::AstNode;
use auto_lsp::lsp_types::{self, Position};
use auto_lsp::tree_sitter::{self, Point};
use auto_lsp::{
    anyhow,
    default::db::{file::File, BaseDatabase},
};
use salsa::Accumulator;

use crate::check::diagnostic_builder::diag;
use crate::check::errors::semantic_errors::{
    incomplete_edge_qualifier, missing_type_for_variable, unauthorized_variable_init,
};
use crate::check::DiagnosticAccumulator;
use crate::hir::expressions::spec::{SimpleSpecKind, Spec, SpecKind};
use crate::hir::interned::identifier::Ident;
use crate::hir::pous::variable::{Variable, VariableKind};
use crate::parser::semantic_index::SemanticIndexBuilder;
use crate::parser::{ParseInit, ParseSpec, ParseSpecInit, ParseVarSection, SpecInitResult};

trait ToVariable<'db> {
    fn to_variable(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        name: &impl AstNode,
        kind: VariableKind,
    ) -> anyhow::Result<Variable<'db>>;
}

impl<'db> ParseVarSection<'db> for ast::generated::InputDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.deref() {
                ast::generated::ERRVariableWithNoSpec_InputVar::ERRVariableWithNoSpec(child) => {
                    missing_type_for_variable(sema.db, child.get_span());
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_InputVar::InputVar(child) => {
                    match child.Type.deref() {
                        ast::generated::InputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Input,
                                )?);
                            }
                        }
                        ast::generated::InputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Input,
                                )?);
                            }
                        }
                        ast::generated::InputVarKind::EdgeDecl(edge_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(edge_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Input,
                                )?);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::FbInputDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.deref() {
                ast::generated::ERRVariableWithNoSpec_FbInputVar::ERRVariableWithNoSpec(child) => {
                    missing_type_for_variable(sema.db, child.get_span());
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_FbInputVar::FbInputVar(child) => {
                    match child.Type.deref() {
                        ast::generated::FbInputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Input,
                                )?);
                            }
                        }
                        ast::generated::FbInputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Input,
                                )?);
                            }
                        }
                        ast::generated::FbInputVarKind::EdgeDecl(edge_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(edge_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Input,
                                )?);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::OutputDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.deref() {
                ast::generated::ERRVariableWithNoSpec_OutputVar::ERRVariableWithNoSpec(child) => {
                    missing_type_for_variable(sema.db, child.get_span());
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_OutputVar::OutputVar(child) => {
                    match child.Type.deref() {
                        ast::generated::OutputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Output,
                                )?);
                            }
                        }
                        ast::generated::OutputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Output,
                                )?);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::FbOutputDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.deref() {
                ast::generated::ERRVariableWithNoSpec_FbOutputVar::ERRVariableWithNoSpec(child) => {
                    missing_type_for_variable(sema.db, child.get_span());
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_FbOutputVar::FbOutputVar(child) => {
                    match child.Type.deref() {
                        ast::generated::FbOutputVarKind::VarDeclInit(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Output,
                                )?);
                            }
                        }
                        ast::generated::FbOutputVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Output,
                                )?);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::TempVarDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.deref() {
                ast::generated::ERRVariableWithNoSpec_TempVar::ERRVariableWithNoSpec(child) => {
                    missing_type_for_variable(sema.db, child.get_span());
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_TempVar::TempVar(child) => {
                    match child.Type.deref() {
                        ast::generated::TempVarKind::VarDecl(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Input,
                                )?);
                            }
                        }
                        ast::generated::TempVarKind::RefSpec(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Input,
                                )?);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::InOutDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.deref() {
                ast::generated::ERRVariableWithNoSpec_InOutVar::ERRVariableWithNoSpec(child) => {
                    missing_type_for_variable(sema.db, child.get_span());
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_InOutVar::InOutVar(child) => {
                    match child.Type.deref() {
                        ast::generated::InOutVarKind::ArrayConformand(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::InOut,
                                )?);
                            }
                        }
                        ast::generated::InOutVarKind::VarDecl(var_decl) => {
                            for variable in child.variables.children.iter() {
                                section.push(var_decl.to_variable(
                                    sema,
                                    variable.deref(),
                                    VariableKind::Input,
                                )?);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'db> ToVariable<'db> for ast::generated::EdgeDecl {
    fn to_variable(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        name: &impl AstNode,
        kind: VariableKind,
    ) -> anyhow::Result<Variable<'db>> {
        let var_name = Ident::from_node(sema.db, sema.file, name)?;
        let result = self.to_spec_init(sema)?;
        let self_span = self.get_span();
        let name_span = name.get_span();
        Ok(Variable::new(
            sema.db,
            sema.file,
            var_name,
            name.get_span(),
            name.get_span(),
            kind,
            result.spec,
            result.init,
            sema.current_scope,
        ))
    }
}

impl<'db> ToVariable<'db> for ast::generated::VarDecl {
    fn to_variable(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        name: &impl AstNode,
        kind: VariableKind,
    ) -> anyhow::Result<Variable<'db>> {
        let var_name = Ident::from_node(sema.db, sema.file, name)?;
        let result = self.to_spec_init(sema)?;
        Ok(Variable::new(
            sema.db,
            sema.file,
            var_name,
            name.get_span(),
            name.get_span(),
            VariableKind::Input,
            result.spec,
            result.init,
            sema.current_scope,
        ))
    }
}

impl<'db> ToVariable<'db> for ast::generated::VarDeclInit {
    fn to_variable(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        name: &impl AstNode,
        kind: VariableKind,
    ) -> anyhow::Result<Variable<'db>> {
        let var_name = Ident::from_node(sema.db, sema.file, name)?;
        let result = self.to_spec_init(sema)?;

        Ok(Variable::new(
            sema.db,
            sema.file,
            var_name,
            name.get_span(),
            name.get_span(),
            VariableKind::Input,
            result.spec,
            result.init,
            sema.current_scope,
        ))
    }
}

impl<'db> ToVariable<'db> for ast::generated::ArrayConformand {
    fn to_variable(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        name: &impl AstNode,
        kind: VariableKind,
    ) -> anyhow::Result<Variable<'db>> {
        let var_name = Ident::from_node(sema.db, sema.file, name)?;
        let result = self.to_spec_init(sema)?;

        Ok(Variable::new(
            sema.db,
            sema.file,
            var_name,
            name.get_span(),
            name.get_span(),
            VariableKind::Input,
            result.spec,
            result.init,
            sema.current_scope,
        ))
    }
}

impl<'db> ToVariable<'db> for ast::generated::LocPartlyVar {
    fn to_variable(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        name: &impl AstNode,
        kind: VariableKind,
    ) -> anyhow::Result<Variable<'db>> {
        let var_name = Ident::from_node(sema.db, sema.file, name)?;
        let result = self.to_spec_init(sema)?;

        Ok(Variable::new(
            sema.db,
            sema.file,
            var_name,
            name.get_span(),
            name.get_span(),
            VariableKind::Input,
            result.spec,
            result.init,
            sema.current_scope,
        ))
    }
}

impl<'db> ToVariable<'db> for ast::generated::RefSpec {
    fn to_variable(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        name: &impl AstNode,
        kind: VariableKind,
    ) -> anyhow::Result<Variable<'db>> {
        let var_name = Ident::from_node(sema.db, sema.file, name)?;
        let result = self.to_spec_init(sema)?;

        Ok(Variable::new(
            sema.db,
            sema.file,
            var_name,
            name.get_span(),
            name.get_span(),
            VariableKind::Input,
            result.spec,
            result.init,
            sema.current_scope,
        ))
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::ExternalVarDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.deref() {
                ast::generated::ERRVariableWithNoSpec_ExternalDecl::ExternalDecl(child) => {
                    match child.Type.deref() {
                        ExternalVarKind::VarDecl(var_decl) => {
                            section.push(var_decl.to_variable(
                                sema,
                                child.name.deref(),
                                VariableKind::Local,
                            )?);
                        }
                        ExternalVarKind::ArrayConformand(var_decl) => {
                            section.push(var_decl.to_variable(
                                sema,
                                child.name.deref(),
                                VariableKind::Local,
                            )?);
                        }
                    }
                }
                ast::generated::ERRVariableWithNoSpec_ExternalDecl::ERRVariableWithNoSpec(
                    child,
                ) => {
                    missing_type_for_variable(sema.db, child.get_span());
                    continue;
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::VarDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.deref() {
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::ERRVariableWithNoSpec(
                    var_decl,
                ) => {
                    missing_type_for_variable(sema.db, child.get_span());
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::VarDeclInitList(
                    var_decl,
                ) => {
                    for variable in var_decl.variables.children.iter() {
                        section.push(var_decl.Type.to_variable(
                            sema,
                            variable.deref(),
                            VariableKind::Local,
                        )?);
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::RetainVarDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.deref() {
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::ERRVariableWithNoSpec(
                    var_decl,
                ) => {
                    missing_type_for_variable(sema.db, child.get_span());
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::VarDeclInitList(
                    var_decl,
                ) => {
                    for variable in var_decl.variables.children.iter() {
                        section.push(var_decl.Type.to_variable(
                            sema,
                            variable.deref(),
                            VariableKind::Local,
                        )?);
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::NoRetainVarDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.deref() {
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::ERRVariableWithNoSpec(
                    var_decl,
                ) => {
                    missing_type_for_variable(sema.db, child.get_span());
                    continue;
                }
                ast::generated::ERRVariableWithNoSpec_VarDeclInitList::VarDeclInitList(
                    var_decl,
                ) => {
                    for variable in var_decl.variables.children.iter() {
                        section.push(var_decl.Type.to_variable(
                            sema,
                            variable.deref(),
                            VariableKind::Local,
                        )?);
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::LocPartlyVarDecl {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            section.push(child.to_variable(
                sema,
                child.variable_name.deref(),
                VariableKind::Local,
            )?);
        }
        Ok(())
    }
}

impl<'db> ParseVarSection<'db> for ast::generated::GlobalVarDecls {
    fn parse(
        &self,
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()> {
        for child in self.children.iter() {
            match child.Type.deref() {
                GlobalVarKind::NamespaceAccess(var_decl) => {
                    let name = Ident::from_node(sema.db, sema.file, child.spec.deref())?;
                    let result = var_decl.to_spec_init(sema)?;
                    section.push(Variable::new(
                        sema.db,
                        sema.file,
                        name,
                        child.get_span(),
                        child.spec.get_span(),
                        VariableKind::Global,
                        result.spec,
                        result.init,
                        sema.current_scope,
                    ))
                }
                GlobalVarKind::LocVarSpecInit(var_decl) => {
                    let name = Ident::from_node(sema.db, sema.file, child.spec.deref())?;
                    let result = var_decl.to_spec_init(sema)?;
                    section.push(Variable::new(
                        sema.db,
                        sema.file,
                        name,
                        child.get_span(),
                        child.spec.get_span(),
                        VariableKind::Global,
                        result.spec,
                        result.init,
                        sema.current_scope,
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
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        let spec = match self.edge.deref() {
            ast::generated::ERRInvalidEdgeQualifier_FEDGE_REDGE::ERRInvalidEdgeQualifier(err) => {
                incomplete_edge_qualifier(sema.db, err.get_span());
                Spec::new(
                    sema.db,
                    self.get_span(),
                    SpecKind::Simple(SimpleSpecKind::Bool),
                    sema.current_scope,
                    sema.file,
                )
            }
            ast::generated::ERRInvalidEdgeQualifier_FEDGE_REDGE::Token_F_EDGE(fedge) => Spec::new(
                sema.db,
                self.get_span(),
                SpecKind::Simple(SimpleSpecKind::Bool),
                sema.current_scope,
                sema.file,
            ),
            ast::generated::ERRInvalidEdgeQualifier_FEDGE_REDGE::Token_R_EDGE(redge) => Spec::new(
                sema.db,
                self.get_span(),
                SpecKind::Simple(SimpleSpecKind::Bool),
                sema.current_scope,
                sema.file,
            ),
        };

        Ok(SpecInitResult::new(spec, None))
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::LocPartlyVar {
    fn to_spec_init(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        todo!()
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::VarDecl {
    fn to_spec_init(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        type Spec = ast::generated::ArrayTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec;

        let spec = match self.spec.deref() {
            Spec::ArrayTypeSpec(a) => a.to_spec(sema),
            Spec::SimpleTypeSpec(a) => a.to_spec(sema),
            Spec::StrTypeSpec(a) => a.to_spec(sema),
            Spec::StructTypeSpec(a) => a.to_spec(sema),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

        let init = match self.init.as_deref() {
            Some(Init::ArrayTypeInit(a)) => Some(a.to_init(sema)?),
            Some(Init::SimpleTypeInit(a)) => Some(a.to_init(sema)?),
            Some(Init::StructTypeInit(a)) => Some(a.to_init(sema)?),
            None => None,
        };

        if let Some(init) = init {
            unauthorized_variable_init(sema.db, init.span(sema.db).clone());
        }

        Ok(SpecInitResult::new(spec?, None))
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::VarDeclInit {
    fn to_spec_init(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        type Spec =
            ast::generated::ArrayTypeSpec_RefTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec;

        let spec = match self.spec.deref() {
            Spec::ArrayTypeSpec(a) => a.to_spec(sema),
            Spec::SimpleTypeSpec(a) => a.to_spec(sema),
            Spec::StrTypeSpec(a) => a.to_spec(sema),
            Spec::StructTypeSpec(a) => a.to_spec(sema),
            Spec::RefTypeSpec(target) => target.to_spec(sema),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;
        let init = match self.init.as_deref() {
            Some(Init::ArrayTypeInit(a)) => Some(a.to_init(sema)?),
            Some(Init::SimpleTypeInit(a)) => Some(a.to_init(sema)?),
            Some(Init::StructTypeInit(a)) => Some(a.to_init(sema)?),
            None => None,
        };

        Ok(SpecInitResult::new(spec?, init))
    }
}

impl<'db> ParseSpecInit<'db> for ast::generated::LocVarSpecInit {
    fn to_spec_init(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        type Spec = ast::generated::ArrayTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec;

        let spec = match self.spec.deref() {
            Spec::ArrayTypeSpec(a) => a.to_spec(sema),
            Spec::SimpleTypeSpec(a) => a.to_spec(sema),
            Spec::StrTypeSpec(a) => a.to_spec(sema),
            Spec::StructTypeSpec(a) => a.to_spec(sema),
        };

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;
        let init = match self.init.as_deref() {
            Some(Init::ArrayTypeInit(a)) => Some(a.to_init(sema)?),
            Some(Init::SimpleTypeInit(a)) => Some(a.to_init(sema)?),
            Some(Init::StructTypeInit(a)) => Some(a.to_init(sema)?),
            None => None,
        };

        Ok(SpecInitResult::new(spec?, init))
    }
}
