use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

use ide_diagnostic::IdeDiagnostic;

use crate::{
    Visibility,
    builder::{
        Parse, ParseSpec, ParseVarSection,
        expression::ParseDirectVariable,
        semantic_index::SemanticIndexBuilder,
    },
    check::errors::ToIdeDiagnostic,
    hir_def::{
        config::{
            AccessDecl, AccessDirection, AccessPath, ConfigDecl, ConfigInstInit, ConfigResource,
            DataSink, DataSource, FbTask, ProgCnxn, ProgConfElement, ProgConfig, ResourceDecl,
            TaskConfig,
        },
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::SpanNamespaceAccess,
        },
        pous::variable::VariableDecl,
        scope::ScopeKind,
    },
};

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_config(
        &mut self,
        config: &ast::generated::ConfigDecl,
    ) -> anyhow::Result<ConfigDecl<'db>, IdeDiagnostic> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let name = Ident::from_node(self.db, self.file, config.name.cast(self.ast))?;

        let mut variables: Vec<VariableDecl<'db>> = vec![];
        if let Some(global_vars) = &config.global_variables {
            global_vars.cast(self.ast).parse(self, &mut variables);
        }

        let mut resources: Vec<ConfigResource<'db>> = vec![];
        for res_id in &config.resources {
            match res_id.cast(self.ast) {
                ast::generated::ResourceDecl_SingleResourceDecl::ResourceDecl(rd) => {
                    let r = self.parse_resource_decl(rd);
                    if let Some(r) = self.try_parse(r) {
                        resources.push(ConfigResource::Resource(r));
                    }
                }
                ast::generated::ResourceDecl_SingleResourceDecl::SingleResourceDecl(srd) => {
                    match srd.children.cast(self.ast) {
                        ast::generated::ProgConfig_TaskConfig::TaskConfig(tc) => {
                            let r = self.parse_task_config(tc);
                            if let Some(t) = self.try_parse(r) {
                                resources.push(ConfigResource::Task(t));
                            }
                        }
                        ast::generated::ProgConfig_TaskConfig::ProgConfig(pc) => {
                            let r = self.parse_prog_config(pc);
                            if let Some(p) = self.try_parse(r) {
                                resources.push(ConfigResource::Program(p));
                            }
                        }
                    }
                }
            }
        }

        let mut access_decls: Vec<AccessDecl<'db>> = vec![];
        if let Some(access_section) = &config.access_decls {
            for decl_id in &access_section.cast(self.ast).children {
                let r = self.parse_access_decl(decl_id.cast(self.ast));
                if let Some(d) = self.try_parse(r) {
                    access_decls.push(d);
                }
            }
        }

        let mut config_init: Vec<ConfigInstInit<'db>> = vec![];
        if let Some(init_section) = &config.config_init {
            for inst_id in &init_section.cast(self.ast).children {
                let r = self.parse_config_inst_init(inst_id.cast(self.ast));
                if let Some(i) = self.try_parse(r) {
                    config_init.push(i);
                }
            }
        }

        let config_decl = ConfigDecl::new(
            self.db,
            name,
            config.name.cast(self.ast).into(),
            config.into(),
            variables,
            resources,
            access_decls,
            config_init,
            scope_id,
        );

        self.register_scope(
            ScopeKind::Config(config_decl),
            vec![],
            scope_id,
            Visibility::empty(),
            previous_scope,
        );

        Ok(config_decl)
    }

    fn parse_resource_decl(
        &mut self,
        rd: &ast::generated::ResourceDecl,
    ) -> anyhow::Result<ResourceDecl<'db>, IdeDiagnostic> {
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
                    let r = self.parse_task_config(tc);
                    if let Some(t) = self.try_parse(r) {
                        tasks.push(t);
                    }
                }
                ast::generated::ProgConfig_TaskConfig::ProgConfig(pc) => {
                    let r = self.parse_prog_config(pc);
                    if let Some(p) = self.try_parse(r) {
                        programs.push(p);
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
    ) -> anyhow::Result<TaskConfig<'db>, IdeDiagnostic> {
        let name = SpanIdent::from_node(self.db, self, tc.name.cast(self.ast))?;
        let init = tc.init.cast(self.ast);

        let single = init.single.as_ref().map(|ds| self.parse_data_source(ds.cast(self.ast)));
        let single = single.and_then(|r| self.try_parse(r));

        let interval = init.interval.as_ref().map(|ds| self.parse_data_source(ds.cast(self.ast)));
        let interval = interval.and_then(|r| self.try_parse(r));

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
    ) -> anyhow::Result<ProgConfig<'db>, IdeDiagnostic> {
        let name = SpanIdent::from_node(self.db, self, pc.name.cast(self.ast))?;

        let retain = pc.retain.is_some();

        let task = pc.task.iter().find_map(|wid| match wid.cast(self.ast) {
            ast::generated::WITH_Identifier::Identifier(ident) => {
                let r = SpanIdent::from_node(self.db, self, ident);
                self.try_parse(r)
            }
            ast::generated::WITH_Identifier::Token_WITH(_) => None,
        });

        let prog_type = SpanNamespaceAccess::from_ast(self.db, self, pc.access.cast(self.ast))?;

        let mut conf_elements: Vec<ProgConfElement<'db>> = vec![];
        if let Some(elems) = &pc.configuration_elements {
            for elem_id in &elems.cast(self.ast).children {
                match elem_id.cast(self.ast).children.cast(self.ast) {
                    ast::generated::FbTask_ProgCnxn::FbTask(fb) => {
                        let r = self.parse_fb_task(fb);
                        if let Some(f) = self.try_parse(r) {
                            conf_elements.push(ProgConfElement::FbTask(f));
                        }
                    }
                    ast::generated::FbTask_ProgCnxn::ProgCnxn(cnxn) => {
                        let r = self.parse_prog_cnxn(cnxn);
                        if let Some(c) = self.try_parse(r) {
                            conf_elements.push(ProgConfElement::Connection(c));
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
    ) -> anyhow::Result<FbTask<'db>, IdeDiagnostic> {
        let path = fb.children.cast(self.ast).parse(self)?;
        let task = Ident::from_node(self.db, self.file, fb.task.cast(self.ast))?;
        Ok(FbTask { path, task })
    }

    fn parse_prog_cnxn(
        &mut self,
        cnxn: &ast::generated::ProgCnxn,
    ) -> anyhow::Result<ProgCnxn<'db>, IdeDiagnostic> {
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
            crate::check::errors::e0_syntax::SyntaxError::InvalidPouKeyword(cnxn.get_span()).to_diagnostic(self.db)
        })?;

        if let Some(source) = source {
            Ok(ProgCnxn::Source { path, source })
        } else if let Some(sink) = sink {
            Ok(ProgCnxn::Sink { path, sink })
        } else {
            Err(crate::check::errors::e0_syntax::SyntaxError::InvalidPouKeyword(cnxn.get_span()).to_diagnostic(self.db))
        }
    }

    fn parse_data_source(
        &mut self,
        ds: &ast::generated::DataSource,
    ) -> anyhow::Result<DataSource<'db>, IdeDiagnostic> {
        match ds.children.cast(self.ast) {
            ast::generated::Constant_DirectVariable_PathExpression::Constant(c) => {
                Ok(DataSource::Constant(c.parse(self)?))
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
    ) -> anyhow::Result<DataSource<'db>, IdeDiagnostic> {
        match pds.children.cast(self.ast) {
            ast::generated::Constant_DirectVariable_PathExpression::Constant(c) => {
                Ok(DataSource::Constant(c.parse(self)?))
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
    ) -> anyhow::Result<DataSink<'db>, IdeDiagnostic> {
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
    ) -> anyhow::Result<AccessDecl<'db>, IdeDiagnostic> {
        let name = Ident::from_node(self.db, self.file, decl.name.cast(self.ast))?;

        let path_node = decl.path.cast(self.ast);
        let path_expr = path_node.path.cast(self.ast).parse(self)?;
        let direct = path_node.direct.as_ref().map(|dv| dv.cast(self.ast).to_direct_variable(self));
        let direct = direct.and_then(|r| self.try_parse(r));
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
    ) -> anyhow::Result<ConfigInstInit<'db>, IdeDiagnostic> {
        use crate::builder::ParseSpecInit;

        let path = inst.path.cast(self.ast).parse(self)?;

        let mut located_at = None;
        let mut init_expr = None;

        for child in &inst.children {
            match child.cast(self.ast) {
                ast::generated::LocVarSpecInit_LocatedAt::LocatedAt(la) => {
                    let r = la.children.cast(self.ast).to_direct_variable(self);
                    located_at = self.try_parse(r);
                }
                ast::generated::LocVarSpecInit_LocatedAt::LocVarSpecInit(lvsi) => {
                    let r = lvsi.to_spec_init(self);
                    if let Some(result) = self.try_parse(r) {
                        init_expr = result.init;
                    }
                }
            }
        }

        let init = init_expr.ok_or_else(|| {
            crate::check::errors::e0_syntax::SyntaxError::InvalidPouKeyword(inst.get_span()).to_diagnostic(self.db)
        })?;

        Ok(crate::hir_def::config::ConfigInstInit {
            path,
            located_at,
            init,
        })
    }
}
