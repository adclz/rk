use std::collections::HashMap;
use std::ops::Deref;

use ast::generated::ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::{BaseDatabase, File};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::hir::namespace::{FileNamespaces, Namespace, Pou, PouDecl};
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
            if let ast::generated::ClassDecl_ConfigDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl_ProgDecl::NamespaceDecl(namespace) =
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

        let mut in_scopes = FxHashSet::default();
        for directive in nested.directives.iter() {
            let path = directive
                .children
                .iter()
                .map(|n| Ident::from_node(self.db, self.file, n.deref()))
                .collect::<anyhow::Result<Vec<_>>>()?;

            let namespace_path = NamespacePath::from((self.db, &path));
            in_scopes.insert(namespace_path);
        }

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
                                Pou::Function(func.parse(self.db, self.file)?),
                                *func.get_range(),
                                name,
                                *func.name.get_range(),
                            ),
                        );
                    }
                    Decl::FbDecl(fb) => {
                        let name = Ident::from_node(self.db, self.file, &*fb.name)?;
                        pous.push(
                            PouDecl::new(
                                self.db,
                                Pou::FunctionBlock(fb.parse(self.db, self.file)?),
                                *fb.get_range(),
                                name,
                                *fb.name.get_range(),
                            ),
                        );
                    }
                    Decl::ClassDecl(class) => {
                        let name = Ident::from_node(self.db, self.file, &*class.name)?;
                        pous.push(
                            PouDecl::new(
                                self.db,
                                Pou::Class(class.parse(self.db, self.file)?),
                                *class.get_range(),
                                name,
                                *class.name.get_range(),
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
                                Pou::Interface(interface.parse(self.db, self.file)?),
                                *interface.get_range(),
                                name,
                                *interface.name.get_range(),
                            ),
                        );
                    },
                }
            }
        }
        Ok(Namespace::new(
            self.db,
            nested.internal.is_some(),
            in_scopes,
            *nested.get_range(),
            NamespacePath::from((self.db, parent_path)),
            *nested.name.get_range(),
            pous,
        ))
    }
}
