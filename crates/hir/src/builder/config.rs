use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

use ide_diagnostic::IdeDiagnostic;

use crate::check::errors::e14_config::ConfigError;
use crate::{
    builder::{
        Parse, ParseSpec, ParseVarSection, expression::ParseDirectVariable,
        semantic_index::SemanticIndexBuilder,
    },
    check::errors::{ToIdeDiagnostic, e00_syntax::SyntaxError},
    hir_def::{
        config::{
            AccessDecl, AccessDirection, AccessPath, ConfigDecl, ConfigInstInit, DataSink,
            DataSource, FbTask, ProgCnxn, ProgConfElement, ProgConfig, ResourceDecl, TaskConfig,
        },
        hir_node::HirNode,
        interned::identifier::{Ident, SpanIdent},
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
        for var in &config.global_variables {
            use ast::generated::ConfigVariables;
            match var.cast(self.ast) {
                ConfigVariables::GlobalVarDecls(decls) => {
                    decls.parse(self, &mut variables);
                }
                ConfigVariables::ERRVarNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ConfigVariables::ERRVarInOutNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarInOutNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ConfigVariables::ERRVarTempNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarTempNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
                ConfigVariables::ERRVarExternalNotAllowed(err) => {
                    self.errors.push(
                        SyntaxError::VarExternalNotAllowed(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
            }
        }

        let mut resources: Vec<ResourceDecl<'db>> = vec![];
        for res_id in &config.resources {
            use ast::generated::ERRTaskOrProgramOutsideResource_ResourceDecl as ConfigEntry;
            match res_id.cast(self.ast) {
                ConfigEntry::ResourceDecl(rd) => {
                    let r = self.parse_resource_decl(rd);
                    if let Some(r) = self.try_parse(r) {
                        resources.push(r);
                    }
                }
                // A TASK or PROGRAM written straight into the CONFIGURATION.
                ConfigEntry::ERRTaskOrProgramOutsideResource(err) => {
                    self.errors.push(
                        ConfigError::TaskOrProgramOutsideResource(err.get_range().to_owned())
                            .to_diagnostic(self.db, self.file),
                    );
                }
            }
        }

        // The sections may appear in any order and any number, so each kind is
        // read as a list and flattened in declaration order.
        let mut access_decls: Vec<AccessDecl<'db>> = vec![];
        for access_section in &config.access_decls {
            for decl_id in &access_section.cast(self.ast).children {
                let r = self.parse_access_decl(decl_id.cast(self.ast));
                if let Some(d) = self.try_parse(r) {
                    access_decls.push(d);
                }
            }
        }

        let mut config_init: Vec<ConfigInstInit<'db>> = vec![];
        for init_section in &config.config_init {
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

        // A RESOURCE groups tasks and programs; it holds no variables. Globals
        // live at the CONFIGURATION (application scope), so a VAR_GLOBAL here
        // parses as an error node and is reported rather than bound.
        if let Some(err) = &rd.global_variables {
            self.errors.push(
                SyntaxError::VarGlobalNotAllowed(err.cast(self.ast).get_range().to_owned())
                    .to_diagnostic(self.db, self.file),
            );
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

        let resource = ResourceDecl::new(
            self.db,
            name,
            resource_type_name,
            tasks,
            programs,
            rd.into(),
            self.current_scope,
        );
        self.register_node(resource.span(self.db), HirNode::Resource(resource));
        Ok(resource)
    }

    fn parse_task_config(
        &mut self,
        tc: &ast::generated::TaskConfig,
    ) -> anyhow::Result<TaskConfig<'db>, IdeDiagnostic> {
        let name = SpanIdent::from_node(self.db, self, tc.name.cast(self.ast))?;
        let init = tc.init.cast(self.ast);

        for err in init.children.iter() {
            match err.cast(self.ast) {
                ast::generated::ERRIntervalAfterPriority_ERRSingleAfterInterval_ERRSingleAfterPriorty::ERRSingleAfterInterval(e) => {
                    self.errors.push(ConfigError::SingleAfterInterval(e.get_range().to_owned()).to_diagnostic(self.db, self.file));
                }
                ast::generated::ERRIntervalAfterPriority_ERRSingleAfterInterval_ERRSingleAfterPriorty::ERRSingleAfterPriorty(e) => {
                    self.errors.push(ConfigError::SingleAfterPriority(e.get_range().to_owned()).to_diagnostic(self.db, self.file));
                }
                ast::generated::ERRIntervalAfterPriority_ERRSingleAfterInterval_ERRSingleAfterPriorty::ERRIntervalAfterPriority(e) => {
                    self.errors.push(ConfigError::IntervalAfterPriority(e.get_range().to_owned()).to_diagnostic(self.db, self.file));
                }
            }
        }

        let single = init
            .single
            .as_ref()
            .map(|ds| self.parse_data_source(ds.cast(self.ast)));
        let single = single.and_then(|r| self.try_parse(r));

        let interval = init
            .interval
            .as_ref()
            .map(|ds| self.parse_data_source(ds.cast(self.ast)));
        let interval = interval.and_then(|r| self.try_parse(r));

        let priority = init
            .priority
            .as_ref()
            .map(|p| Ident::from_node(self.db, self.file, p.cast(self.ast)));
        let priority = priority.and_then(|r| self.try_parse(r));

        if priority.is_none() {
            self.errors.push(
                ConfigError::MissingPriority(tc.get_range().to_owned())
                    .to_diagnostic(self.db, self.file),
            );
        }

        let task = TaskConfig::new(
            self.db,
            name,
            single,
            interval,
            priority,
            tc.into(),
            self.current_scope,
        );
        self.register_node(task.span(self.db), HirNode::Task(task));
        Ok(task)
    }

    fn parse_prog_config(
        &mut self,
        pc: &ast::generated::ProgConfig,
    ) -> anyhow::Result<ProgConfig<'db>, IdeDiagnostic> {
        let name = SpanIdent::from_node(self.db, self, pc.name.cast(self.ast))?;

        // Tri-state: RETAIN => Some(true), NON_RETAIN => Some(false), absent
        // => None. (`is_some()` here was a latent bug — it read `PROGRAM
        // NON_RETAIN` as retained.)
        let retain = pc.retain.as_ref().map(|r| {
            matches!(
                r.cast(self.ast),
                ast::generated::Operators_3::Token_RETAIN(_)
            )
        });

        let task = pc.task.iter().find_map(|wid| match wid.cast(self.ast) {
            ast::generated::WITH_Identifier::Identifier(ident) => {
                let r = SpanIdent::from_node(self.db, self, ident);
                self.try_parse(r)
            }
            ast::generated::WITH_Identifier::Token_WITH(_) => None,
        });

        let prog_type = pc.access.cast(self.ast).to_spec(self)?;

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

        let prog = ProgConfig::new(
            self.db,
            name,
            retain,
            task,
            prog_type,
            conf_elements,
            pc.into(),
            self.current_scope,
        );
        self.register_node(prog.span(self.db), HirNode::ProgConfig(prog));
        Ok(prog)
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

        // The grammar's `prog_cnxn` rule requires a path expression
        // plus exactly one of (source, sink). Partial/recovered parses
        // can still leave us short, so surface that as a syntax error
        // (E0002) rather than panic.
        let missing = |what: &str| {
            crate::check::errors::e00_syntax::SyntaxError::MissingNode {
                file: self.file,
                span: cnxn.get_range().to_owned(),
                err: format!("prog_cnxn missing {what}"),
                grammar_name: "prog_cnxn",
            }
            .to_diagnostic(self.db, self.file)
        };

        let path = path.ok_or_else(|| missing("path expression"))?;

        // An address a connection names is mentioned like one in a body, and
        // gets a cell the same way.
        let direct = match (&source, &sink) {
            (Some(DataSource::Direct(dv)), _) | (_, Some(DataSink::Direct(dv))) => Some(*dv),
            _ => None,
        };
        if let Some(address) =
            direct.and_then(|dv| crate::hir_def::pous::variable::LocatedAddress::of(self.db, dv))
        {
            self.located.push((address, HirNode::PathExpr(path)));
        }

        if let Some(source) = source {
            Ok(ProgCnxn::Source { path, source })
        } else if let Some(sink) = sink {
            Ok(ProgCnxn::Sink { path, sink })
        } else {
            Err(missing("source or sink"))
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
        let direct = path_node
            .direct
            .as_ref()
            .map(|dv| dv.cast(self.ast).to_direct_variable(self));
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
        let mut spec = None;

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
                        spec = Some(result.spec);
                    }
                }
            }
        }

        // Grammar normally guarantees an init expression on a config
        // inst declaration; partial/recovered parses may still leave
        // us short, so surface that as a syntax error (E0002).
        // Only an entry with neither a location nor a value is malformed.
        if located_at.is_none() && init_expr.is_none() {
            return Err(crate::check::errors::e00_syntax::SyntaxError::MissingNode {
                file: self.file,
                span: inst.get_range().to_owned(),
                err: "a VAR_CONFIG entry needs a location (AT) or an initial value".into(),
                grammar_name: "config_inst_init",
            }
            .to_diagnostic(self.db, self.file));
        }

        Ok(crate::hir_def::config::ConfigInstInit {
            path,
            located_at,
            spec,
            init: init_expr,
        })
    }
}
