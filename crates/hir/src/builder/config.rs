use std::sync::Arc;

use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

use crate::{
    Visibility,
    builder::{
        ParseSpec, ParseVarSection,
        expression::{ParseDirectVariable, ParseExpr, ParseExpression},
        semantic_index::SemanticIndexBuilder,
    },
    check::errors::analysis_error::AnalysisError,
    hir_def::{
        config::{
            AccessDecl, AccessDirection, AccessPath, ConfigDecl, ConfigInstInit, ConfigResource,
            DataSink, DataSource, FbTask, ProgCnxn, ProgConfElement, ProgConfig, ResourceDecl,
            TaskConfig,
        },
        interned::{identifier::{Ident, SpanIdent}, namespace::SpanNamespaceAccess},
        pous::variable::VariableDecl,
        scope::{Scope, ScopeKind},
    },
};

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_config(
        &mut self,
        config: &ast::generated::ConfigDecl,
    ) -> anyhow::Result<ConfigDecl<'db>, AnalysisError<'db>> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let name = Ident::from_node(self.db, self.file, config.name.cast(self.ast))?;

        // Parse VAR_GLOBAL section at the configuration level.
        let mut variables: Vec<VariableDecl<'db>> = vec![];
        if let Some(global_vars) = &config.global_variables {
            global_vars.cast(self.ast).parse(self, &mut variables);
        }

        // Parse RESOURCE / single_resource_decl entries.
        let mut resources: Vec<ConfigResource<'db>> = vec![];
        for res_id in &config.resources {
            match res_id.cast(self.ast) {
                ast::generated::ResourceDecl_SingleResourceDecl::ResourceDecl(rd) => {
                    match self.parse_resource_decl(rd) {
                        Ok(r) => resources.push(ConfigResource::Resource(r)),
                        Err(err) => self.errors.push(err),
                    }
                }
                ast::generated::ResourceDecl_SingleResourceDecl::SingleResourceDecl(srd) => {
                    match srd.children.cast(self.ast) {
                        ast::generated::ProgConfig_TaskConfig::TaskConfig(tc) => {
                            match self.parse_task_config(tc) {
                                Ok(t) => resources.push(ConfigResource::Task(t)),
                                Err(err) => self.errors.push(err),
                            }
                        }
                        ast::generated::ProgConfig_TaskConfig::ProgConfig(pc) => {
                            match self.parse_prog_config(pc) {
                                Ok(p) => resources.push(ConfigResource::Program(p)),
                                Err(err) => self.errors.push(err),
                            }
                        }
                    }
                }
            }
        }

        // Parse VAR_ACCESS section.
        let mut access_decls: Vec<AccessDecl<'db>> = vec![];
        if let Some(access_section) = &config.access_decls {
            for decl_id in &access_section.cast(self.ast).children {
                match self.parse_access_decl(decl_id.cast(self.ast)) {
                    Ok(d) => access_decls.push(d),
                    Err(err) => self.errors.push(err),
                }
            }
        }

        // Parse VAR_CONFIG section.
        let mut config_init: Vec<ConfigInstInit<'db>> = vec![];
        if let Some(init_section) = &config.config_init {
            for inst_id in &init_section.cast(self.ast).children {
                match self.parse_config_inst_init(inst_id.cast(self.ast)) {
                    Ok(i) => config_init.push(i),
                    Err(err) => self.errors.push(err),
                }
            }
        }

        let config_decl = ConfigDecl::new(
            self.db,
            name,
            config.name.cast(self.ast).into(), // name_span: just the identifier
            config.into(),                      // span: full declaration
            variables,
            resources,
            access_decls,
            config_init,
            scope_id,
        );

        let scope = Scope::new(
            self.file,
            ScopeKind::Config(config_decl),
            vec![],
            scope_id,
            Visibility::empty(),
            Some(previous_scope),
        );

        self.scope_keys
            .insert(scope_id.scope(self.db), Arc::new(scope));

        Ok(config_decl)
    }

    fn parse_resource_decl(
        &mut self,
        rd: &ast::generated::ResourceDecl,
    ) -> anyhow::Result<ResourceDecl<'db>, AnalysisError<'db>> {
        let name = SpanIdent::from_node(self.db, self, rd.name.cast(self.ast))?;
        let resource_type_name =
            Ident::from_node(self.db, self.file, rd.resource_type_name.cast(self.ast))?;

        let mut variables: Vec<VariableDecl<'db>> = vec![];
        if let Some(global_vars) = &rd.global_variables {
            global_vars.cast(self.ast).parse(self, &mut variables);
        }

        let mut tasks: Vec<TaskConfig<'db>> = vec![];
        let mut programs: Vec<ProgConfig<'db>> = vec![];
        for srd_id in &rd.resource {
            match srd_id.cast(self.ast).children.cast(self.ast) {
                ast::generated::ProgConfig_TaskConfig::TaskConfig(tc) => {
                    match self.parse_task_config(tc) {
                        Ok(t) => tasks.push(t),
                        Err(err) => self.errors.push(err),
                    }
                }
                ast::generated::ProgConfig_TaskConfig::ProgConfig(pc) => {
                    match self.parse_prog_config(pc) {
                        Ok(p) => programs.push(p),
                        Err(err) => self.errors.push(err),
                    }
                }
            }
        }

        Ok(ResourceDecl {
            name,
            resource_type_name,
            variables,
            tasks,
            programs,
        })
    }

    fn parse_task_config(
        &mut self,
        tc: &ast::generated::TaskConfig,
    ) -> anyhow::Result<TaskConfig<'db>, AnalysisError<'db>> {
        let name = SpanIdent::from_node(self.db, self, tc.name.cast(self.ast))?;
        let init = tc.init.cast(self.ast);

        let single = init
            .single
            .as_ref()
            .and_then(|ds| match self.parse_data_source(ds.cast(self.ast)) {
                Ok(s) => Some(s),
                Err(err) => {
                    self.errors.push(err);
                    None
                }
            });

        let interval =
            init.interval
                .as_ref()
                .and_then(|ds| match self.parse_data_source(ds.cast(self.ast)) {
                    Ok(s) => Some(s),
                    Err(err) => {
                        self.errors.push(err);
                        None
                    }
                });

        let priority = Ident::from_node(self.db, self.file, init.priority.cast(self.ast))?;

        Ok(TaskConfig {
            name,
            single,
            interval,
            priority,
        })
    }

    fn parse_prog_config(
        &mut self,
        pc: &ast::generated::ProgConfig,
    ) -> anyhow::Result<ProgConfig<'db>, AnalysisError<'db>> {
        let name = SpanIdent::from_node(self.db, self, pc.name.cast(self.ast))?;

        let retain = pc.retain.is_some();

        // `task` is modelled as Vec<WITH_Identifier>: find the Identifier variant.
        let task = pc.task.iter().find_map(|wid| {
            match wid.cast(self.ast) {
                ast::generated::WITH_Identifier::Identifier(ident) => {
                    match SpanIdent::from_node(self.db, self, ident) {
                        Ok(i) => Some(i),
                        Err(err) => {
                            self.errors.push(err);
                            None
                        }
                    }
                }
                ast::generated::WITH_Identifier::Token_WITH(_) => None,
            }
        });

        let prog_type =
            SpanNamespaceAccess::from_ast(self.db, self, pc.access.cast(self.ast))?;

        let mut conf_elements: Vec<ProgConfElement<'db>> = vec![];
        if let Some(elems) = &pc.configuration_elements {
            for elem_id in &elems.cast(self.ast).children {
                match elem_id.cast(self.ast).children.cast(self.ast) {
                    ast::generated::FbTask_ProgCnxn::FbTask(fb) => {
                        match self.parse_fb_task(fb) {
                            Ok(f) => conf_elements.push(ProgConfElement::FbTask(f)),
                            Err(err) => self.errors.push(err),
                        }
                    }
                    ast::generated::FbTask_ProgCnxn::ProgCnxn(cnxn) => {
                        match self.parse_prog_cnxn(cnxn) {
                            Ok(c) => conf_elements.push(ProgConfElement::Connection(c)),
                            Err(err) => self.errors.push(err),
                        }
                    }
                }
            }
        }

        Ok(ProgConfig {
            retain,
            name,
            task,
            prog_type,
            conf_elements,
        })
    }

    fn parse_fb_task(
        &mut self,
        fb: &ast::generated::FbTask,
    ) -> anyhow::Result<FbTask<'db>, AnalysisError<'db>> {
        let path = fb.children.cast(self.ast).parse(self)?;
        let task = Ident::from_node(self.db, self.file, fb.task.cast(self.ast))?;
        Ok(FbTask { path, task })
    }

    fn parse_prog_cnxn(
        &mut self,
        cnxn: &ast::generated::ProgCnxn,
    ) -> anyhow::Result<ProgCnxn<'db>, AnalysisError<'db>> {
        // children: [PathExpression, ProgDataSource | DataSink]
        // The first PathExpression is the LHS; the second entry determines direction.
        let mut path = None;
        let mut source = None;
        let mut sink = None;

        for child in &cnxn.children {
            match child.cast(self.ast) {
                ast::generated::DataSink_PathExpression_ProgDataSource::PathExpression(pe) => {
                    if path.is_none() {
                        path = Some(pe.parse(self)?);
                    }
                }
                ast::generated::DataSink_PathExpression_ProgDataSource::ProgDataSource(pds) => {
                    source = Some(self.parse_prog_data_source(pds)?);
                }
                ast::generated::DataSink_PathExpression_ProgDataSource::DataSink(ds) => {
                    sink = Some(self.parse_data_sink(ds)?);
                }
            }
        }

        let path = path.ok_or_else(|| {
            AnalysisError::Syntax(crate::check::errors::e0_syntax::SyntaxError::InvalidPouKeyword(
                cnxn.get_span(),
            ))
        })?;

        if let Some(source) = source {
            Ok(ProgCnxn::Source { path, source })
        } else if let Some(sink) = sink {
            Ok(ProgCnxn::Sink { path, sink })
        } else {
            Err(AnalysisError::Syntax(
                crate::check::errors::e0_syntax::SyntaxError::InvalidPouKeyword(cnxn.get_span()),
            ))
        }
    }

    fn parse_data_source(
        &mut self,
        ds: &ast::generated::DataSource,
    ) -> anyhow::Result<DataSource<'db>, AnalysisError<'db>> {
        match ds.children.cast(self.ast) {
            ast::generated::Constant_DirectVariable_PathExpression::Constant(c) => {
                Ok(DataSource::Constant(c.to_expr(self)?))
            }
            ast::generated::Constant_DirectVariable_PathExpression::DirectVariable(dv) => {
                Ok(DataSource::Direct(dv.to_direct_variable(self)?))
            }
            ast::generated::Constant_DirectVariable_PathExpression::PathExpression(pe) => {
                Ok(DataSource::Path(pe.parse(self)?))
            }
        }
    }

    fn parse_prog_data_source(
        &mut self,
        pds: &ast::generated::ProgDataSource,
    ) -> anyhow::Result<DataSource<'db>, AnalysisError<'db>> {
        match pds.children.cast(self.ast) {
            ast::generated::Constant_DirectVariable_PathExpression::Constant(c) => {
                Ok(DataSource::Constant(c.to_expr(self)?))
            }
            ast::generated::Constant_DirectVariable_PathExpression::DirectVariable(dv) => {
                Ok(DataSource::Direct(dv.to_direct_variable(self)?))
            }
            ast::generated::Constant_DirectVariable_PathExpression::PathExpression(pe) => {
                Ok(DataSource::Path(pe.parse(self)?))
            }
        }
    }

    fn parse_data_sink(
        &mut self,
        ds: &ast::generated::DataSink,
    ) -> anyhow::Result<DataSink<'db>, AnalysisError<'db>> {
        match ds.children.cast(self.ast) {
            ast::generated::DirectVariable_PathExpression::DirectVariable(dv) => {
                Ok(DataSink::Direct(dv.to_direct_variable(self)?))
            }
            ast::generated::DirectVariable_PathExpression::PathExpression(pe) => {
                Ok(DataSink::Path(pe.parse(self)?))
            }
        }
    }

    fn parse_access_decl(
        &mut self,
        decl: &ast::generated::AccessDecl,
    ) -> anyhow::Result<AccessDecl<'db>, AnalysisError<'db>> {
        let name = Ident::from_node(self.db, self.file, decl.name.cast(self.ast))?;

        let path_node = decl.path.cast(self.ast);
        let path_expr = path_node.path.cast(self.ast).parse(self)?;
        let direct = path_node
            .direct
            .as_ref()
            .and_then(|dv| match dv.cast(self.ast).to_direct_variable(self) {
                Ok(d) => Some(d),
                Err(err) => {
                    self.errors.push(err);
                    None
                }
            });
        let path = AccessPath {
            path: path_expr,
            direct,
        };

        let access = decl.access.cast(self.ast).to_spec(self)?;

        let direction = match &decl.direction {
            None => AccessDirection::ReadWrite,
            Some(dir) => match dir.cast(self.ast).children.cast(self.ast) {
                ast::generated::ReadOnly_ReadWrite::ReadOnly(_) => AccessDirection::ReadOnly,
                ast::generated::ReadOnly_ReadWrite::ReadWrite(_) => AccessDirection::ReadWrite,
            },
        };

        Ok(AccessDecl {
            name,
            path,
            access,
            direction,
        })
    }

    fn parse_config_inst_init(
        &mut self,
        inst: &ast::generated::ConfigInstInit,
    ) -> anyhow::Result<ConfigInstInit<'db>, AnalysisError<'db>> {
        use crate::builder::ParseSpecInit;

        let path = inst.path.cast(self.ast).parse(self)?;

        // children: [optional(LocatedAt), LocVarSpecInit]
        let mut located_at = None;
        let mut init_expr = None;

        for child in &inst.children {
            match child.cast(self.ast) {
                ast::generated::LocVarSpecInit_LocatedAt::LocatedAt(la) => {
                    match la.children.cast(self.ast).to_direct_variable(self) {
                        Ok(dv) => located_at = Some(dv),
                        Err(err) => self.errors.push(err),
                    }
                }
                ast::generated::LocVarSpecInit_LocatedAt::LocVarSpecInit(lvsi) => {
                    match lvsi.to_spec_init(self) {
                        Ok(result) => init_expr = result.init,
                        Err(err) => self.errors.push(err),
                    }
                }
            }
        }

        let init = init_expr.ok_or_else(|| {
            AnalysisError::Syntax(crate::check::errors::e0_syntax::SyntaxError::InvalidPouKeyword(
                inst.get_span(),
            ))
        })?;

        Ok(crate::hir_def::config::ConfigInstInit {
            path,
            located_at,
            init,
        })
    }
}
