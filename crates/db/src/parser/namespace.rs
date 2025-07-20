use std::collections::HashMap;
use std::ops::Deref;
use std::panic;
use std::sync::Arc;

use ast::generated::ERRInvalidPouKeyword_ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::{file::File, BaseDatabase};
use rustc_hash::FxHashMap;
use salsa::Accumulator;

use crate::diagnostics::diagnostic_builder::diag;
use crate::diagnostics::DiagnosticAccumulator;
use crate::hir::namespace::{FileNamespaces, Namespace, Pou, PouDecl, Using};
use crate::ident::{Ident, SpannedIdent};
use crate::parser::data_type::ParseDataType;
use crate::parser::Parse;
use crate::solver::namespace::NamespacePath;

pub struct FileNamespacesBuilder<'db> {
    db: &'db dyn BaseDatabase,
    source: &'db ast::generated::SourceFile,
    pub(crate) file: File,
    pub(crate) directives: Vec<Using<'db>>,
    pub(crate) globals: Vec<PouDecl<'db>>,
    pub(crate) paths: Vec<Namespace<'db>>,
}

pub trait ParseUsing<'db> {
    type Output;

    fn parse_using(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<Self::Output>;
}

impl<'db> ParseUsing<'db> for ast::generated::UsingDirective {
    type Output = Vec<Using<'db>>;

    fn parse_using(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<Self::Output> {
        let mut using = vec![];
        for child in self.children.iter() {
            let mut path = vec![];
            for child in child.children.iter() {
                path.push(SpannedIdent::new(db, file, child.deref())?);
            }
            using.push(Using::new(
                db,
                NamespacePath::from((db, &path)),
                child.get_span(),
            ));
        }
        Ok(using)
    }
}

impl<'db> ParseUsing<'db> for Vec<Arc<ast::generated::UsingDirective>> {
    type Output = Vec<Using<'db>>;
    fn parse_using(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<Self::Output> {
        let mut using = vec![];
        for directive in self.iter() {
            for child in directive.children.iter() {
                let mut path = vec![];
                for child in child.children.iter() {
                    path.push(SpannedIdent::new(db, file, child.deref())?);
                }
                using.push(Using::new(
                    db,
                    NamespacePath::from((db, &path)),
                    child.get_span(),
                ));
            }
        }
        Ok(using)
    }
}

impl<'db> FileNamespacesBuilder<'db> {
    pub fn new(
        db: &'db dyn BaseDatabase,
        file: File,
        source: &'db ast::generated::SourceFile,
    ) -> Self {
        Self {
            db,
            file,
            source,
            directives: vec![],
            globals: vec![],
            paths: vec![],
        }
    }

    pub fn get_namespace_path(
        &mut self,
        namespace: &ast::generated::NamespaceDecl,
    ) -> anyhow::Result<Vec<SpannedIdent>> {
        namespace
            .name
            .children
            .iter()
            .map(|n| SpannedIdent::new(self.db, self.file, n.deref()))
            .collect::<anyhow::Result<Vec<_>>>()
    }

    // Fix me: This function should not panic, but handle errors gracefully.
    pub fn build(mut self) -> FileNamespaces<'db> {
        for child in self.source.children.iter() {
            type SourceFileDecl = ast::generated::ERRInvalidPouKeyword_ClassDecl_ConfigDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl_ProgDecl_UsingDirective;

            match child.as_ref() {
                SourceFileDecl::ERRInvalidPouKeyword(err) => {
                    let diag = diag()
                        .file(self.file)
                        .message("Expected a POU keyword".into())
                        .range(err.get_span())
                        .call();
                    DiagnosticAccumulator::accumulate(diag.into(), self.db);
                }
                SourceFileDecl::NamespaceDecl(namespace) => {
                    let path = match self.get_namespace_path(namespace) {
                        Ok(path) => path,
                        Err(_err) => {
                            panic!("Failed to build namespace: {:?}", _err);
                            continue;
                        }
                    };
                    let namespace_path = NamespacePath::from((self.db, &path));
                    let namespace = match self.handle_namespace_elements(&path, namespace) {
                        Ok(namespace) => namespace,
                        Err(_err) => {
                            panic!("Failed to build namespace: {:?}", _err);
                            continue;
                        }
                    };

                    self.paths.push(namespace);
                }
                SourceFileDecl::UsingDirective(directive) => {
                    let using = directive.parse_using(self.db, self.file).unwrap();
                    self.directives.extend(using);
                }
                SourceFileDecl::FuncDecl(func) => {
                    let name = Ident::from_node(self.db, self.file, &*func.name).unwrap();
                    self.globals.push(PouDecl::new(
                        self.db,
                        self.file,
                        Pou::Function(func.parse(self.db, self.file).unwrap()),
                        func.get_span(),
                        name,
                        func.name.get_span(),
                    ));
                }
                SourceFileDecl::FbDecl(fb) => {
                    let name = Ident::from_node(self.db, self.file, &*fb.name).unwrap();
                    self.globals.push(PouDecl::new(
                        self.db,
                        self.file,
                        Pou::FunctionBlock(fb.parse(self.db, self.file).unwrap()),
                        fb.get_span(),
                        name,
                        fb.name.get_span(),
                    ));
                }
                SourceFileDecl::ClassDecl(class) => {
                    let name = Ident::from_node(self.db, self.file, &*class.name).unwrap();
                    self.globals.push(PouDecl::new(
                        self.db,
                        self.file,
                        Pou::Class(class.parse(self.db, self.file).unwrap()),
                        class.get_span(),
                        name,
                        class.name.get_span(),
                    ));
                }
                SourceFileDecl::DataTypeDecl(data_type) => {
                    data_type
                        .parse(self.db, self.file, &mut self.globals)
                        .unwrap();
                }
                SourceFileDecl::InterfaceDecl(interface) => {
                    let name = Ident::from_node(self.db, self.file, &*interface.name).unwrap();
                    self.globals.push(PouDecl::new(
                        self.db,
                        self.file,
                        Pou::Interface(interface.parse(self.db, self.file).unwrap()),
                        interface.get_span(),
                        name,
                        interface.name.get_span(),
                    ));
                }
                _ => {
                    //todo: add config and program declarations
                }
            }
        }
        FileNamespaces::new(self.db, self.file, self.globals, self.paths)
    }

    pub fn handle_namespace_elements(
        &mut self,
        parent_path: &[SpannedIdent],
        nested: &'db ast::generated::NamespaceDecl,
    ) -> anyhow::Result<Namespace<'db>> {
        type Decl =
            ERRInvalidPouKeyword_ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;

        let using = nested.directives.parse_using(self.db, self.file)?;

        let mut pous = vec![];
        if let Some(elements) = nested.elements.as_ref() {
            for child in elements.children.iter() {
                match child.as_ref() {
                    Decl::NamespaceDecl(namespace) => {
                        let mut path = parent_path.to_vec();
                        path.extend(self.get_namespace_path(namespace)?);

                        let namespace_path: NamespacePath = NamespacePath::from((self.db, &path));
                        let namespace = self.handle_namespace_elements(&path, namespace)?;
                        self.paths.push(namespace);
                    }
                    Decl::FuncDecl(func) => {
                        let name = Ident::from_node(self.db, self.file, &*func.name)?;
                        pous.push(PouDecl::new(
                            self.db,
                            self.file,
                            Pou::Function(func.parse(self.db, self.file)?),
                            func.get_span(),
                            name,
                            func.name.get_span(),
                        ));
                    }
                    Decl::FbDecl(fb) => {
                        let name = Ident::from_node(self.db, self.file, &*fb.name)?;
                        pous.push(PouDecl::new(
                            self.db,
                            self.file,
                            Pou::FunctionBlock(fb.parse(self.db, self.file)?),
                            fb.get_span(),
                            name,
                            fb.name.get_span(),
                        ));
                    }
                    Decl::ClassDecl(class) => {
                        let name = Ident::from_node(self.db, self.file, &*class.name)?;
                        pous.push(PouDecl::new(
                            self.db,
                            self.file,
                            Pou::Class(class.parse(self.db, self.file)?),
                            class.get_span(),
                            name,
                            class.name.get_span(),
                        ));
                    }
                    Decl::DataTypeDecl(data_type) => {
                        data_type.parse(self.db, self.file, &mut pous)?;
                    }
                    Decl::InterfaceDecl(interface) => {
                        let name = Ident::from_node(self.db, self.file, &*interface.name)?;
                        pous.push(PouDecl::new(
                            self.db,
                            self.file,
                            Pou::Interface(interface.parse(self.db, self.file)?),
                            interface.get_span(),
                            name,
                            interface.name.get_span(),
                        ));
                    }
                    Decl::ERRInvalidPouKeyword(err) => {
                        let diag = diag()
                            .file(self.file)
                            .message("Expected a POU keyword".into())
                            .range(err.get_span())
                            .call();
                        DiagnosticAccumulator::accumulate(diag.into(), self.db);
                    }
                }
            }
        }
        Ok(Namespace::new(
            self.db,
            nested.internal.is_some(),
            using,
            nested.get_span(),
            NamespacePath::from((self.db, parent_path)),
            nested.name.get_span(),
            pous,
        ))
    }
}
