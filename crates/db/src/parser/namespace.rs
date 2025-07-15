use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use ast::generated::ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::{BaseDatabase, file::File};
use rustc_hash::FxHashMap;

use crate::hir::namespace::{FileNamespaces, Namespace, Pou, PouDecl, Using};
use crate::ident::Ident;
use crate::parser::Parse;
use crate::solver::namespace::NamespacePath;
use crate::parser::data_type::ParseDataType;

pub struct FileNamespacesBuilder<'db> {
    db: &'db dyn BaseDatabase,
    source: &'db ast::generated::SourceFile,
    pub(crate) file: File,
    pub(crate) paths: FxHashMap<NamespacePath, Namespace<'db>>,
}

pub trait ParseUsing<'db> {
    fn parse_using(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<Vec<Using<'db>>>;
}

impl<'db> ParseUsing<'db> for Vec<Arc<ast::generated::UsingDirective>> {
    fn parse_using(
        &'db self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<Vec<Using<'db>>> {
        let mut using = vec![];
        for directive in self.iter() {
            for child in directive.children.iter() {
                let mut path = vec![];
                for child in child.children.iter() {
                    path.push(Ident::from_node(db, file, child.deref())?);
                }
                using.push(
                    Using::new(
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
            paths: HashMap::default(),
        }
    }

    pub fn get_namespace_path(
        &mut self,
        namespace: &ast::generated::NamespaceDecl,
    ) -> anyhow::Result<Vec<Ident>> {
        namespace
            .name
            .children
            .iter()
            .map(|n| Ident::from_node(self.db, self.file, n.deref()))
            .collect::<anyhow::Result<Vec<_>>>()
    }

    pub fn build(mut self) -> FileNamespaces<'db> {
        // Top level namespaces
        // Since namespaces can be nested, we check
        for child in self.source.children.iter() {
            if let ast::generated::ClassDecl_ConfigDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl_ProgDecl_UsingDirective::NamespaceDecl(namespace) =
                child.as_ref()
            {
                let path = match self.get_namespace_path(namespace) {
                    Ok(path) => path,
                    Err(_err) => {
                        // todo: report error
                        continue;
                    }
                };
                let namespace_path = NamespacePath::from((self.db, &path));
                let namespace = match self.handle_namespace_elements(&path, namespace) {
                    Ok(namespace) => namespace,
                    Err(_err) => {
                        // todo: report error
                        continue;
                    }
                };

                self.paths.entry(namespace_path).or_insert(namespace);
            }
        }
        FileNamespaces::new(self.db, self.file, self.paths)
    }

    pub fn handle_namespace_elements(
        &mut self,
        parent_path: &[Ident],
        nested: &'db ast::generated::NamespaceDecl,
    ) -> anyhow::Result<Namespace<'db>> {
        type Decl = ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;

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
                        self.paths.entry(namespace_path).or_insert(namespace);
                    }
                    Decl::FuncDecl(func) => {
                        let name = Ident::from_node(self.db, self.file, &*func.name)?;
                        pous.push(
                            PouDecl::new(
                                self.db,
                                self.file,
                                Pou::Function(func.parse(self.db, self.file)?),
                                func.get_span(),
                                name,
                                func.name.get_span(),
                            ),
                        );
                    }
                    Decl::FbDecl(fb) => {
                        let name = Ident::from_node(self.db, self.file, &*fb.name)?;
                        pous.push(
                            PouDecl::new(
                                self.db,
                                self.file,
                                Pou::FunctionBlock(fb.parse(self.db, self.file)?),
                                fb.get_span(),
                                name,
                                fb.name.get_span(),
                            ),
                        );
                    }
                    Decl::ClassDecl(class) => {
                        let name = Ident::from_node(self.db, self.file, &*class.name)?;
                        pous.push(
                            PouDecl::new(
                                self.db,
                                self.file,
                                Pou::Class(class.parse(self.db, self.file)?),
                                class.get_span(),
                                name,
                                class.name.get_span(),
                            ),
                        );
                    },
                    Decl::DataTypeDecl(data_type) => {
                        data_type.parse(self.db, self.file, &mut pous)?;
                    },
                    Decl::InterfaceDecl(interface) => {
                        let name = Ident::from_node(self.db, self.file, &*interface.name)?;
                        pous.push(
                            PouDecl::new(
                                self.db,
                                self.file,
                                Pou::Interface(interface.parse(self.db, self.file)?),
                                interface.get_span(),
                                name,
                                interface.name.get_span(),
                            ),
                        );
                    },
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
