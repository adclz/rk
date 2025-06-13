use std::collections::HashMap;

use ast::generated::ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::{BaseDatabase, File};
use rustc_hash::FxHashMap;

use crate::hir::namespace::{FileNamespaces, Namespace,
};
use crate::parser::Parse;
use crate::solver::{Ident, NamespacePath};

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

    pub fn build(mut self) -> FileNamespaces<'db> {
        // Top level namespaces
        // Since namespaces can be nested, we check
        self.source
            .children
            .iter()
            .for_each(|child| if let ast::generated::ConfigDecl_NamespaceDecl_ProgDecl::NamespaceDecl(namespace) = child.as_ref() {
                let path = self.get_namespace_path(namespace);
                let namespace_path = NamespacePath::from((self.db, &path));
                self.paths
                    .entry(namespace_path)
                    .or_insert(Namespace::new(namespace));

                self.handle_namespace_elements(&path, namespace);
            });
        FileNamespaces::new(self)
    }

    pub fn get_namespace_path(&mut self, namespace: &ast::generated::NamespaceDecl) -> Vec<Ident> {
        namespace
            .name
            .children
            .iter()
            .map(|n| {
                let doc = self.file.document(self.db);
                let text = doc.texter.text.as_bytes();
                Ident::new(self.db, n.get_text(text).unwrap().to_string())
            })
            .collect::<Vec<_>>()
    }

    pub fn handle_namespace_elements(
        &mut self,
        parent_path: &[Ident],
        nested: &ast::generated::NamespaceDecl,
    ) {
        type Decl = ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;

        let mut current_namespace = None;

        if !nested.directive.is_empty() {
            nested.directive.iter().for_each(|directive| {
                let path = directive
                    .children
                    .iter()
                    .map(|n| {
                        let doc = self.file.document(self.db);
                        let text = doc.texter.text.as_bytes();
                        Ident::new(self.db, n.get_text(text).unwrap().to_string())
                    })
                    .collect::<Vec<_>>();

                let namespace_path = NamespacePath::from((self.db, &path));
                self.paths
                    .entry(namespace_path)
                    .or_insert(Namespace::new(nested))
                    .in_scopes
                    .insert(NamespacePath::from((self.db, parent_path)));
            })
        }
        
        if let Some(elements) = nested.elements.as_ref() {
            elements
            .children
            .iter()
            .for_each(|child| match child.as_ref() {
                Decl::NamespaceDecl(namespace) => {
                    let mut path = parent_path.to_vec();
                    path.extend(self.get_namespace_path(namespace));

                    let namespace_path: NamespacePath = NamespacePath::from((self.db, &path));
                    self.paths
                        .entry(namespace_path)
                        .or_insert(Namespace::new(namespace));
                    current_namespace = Some(namespace_path);

                    self.handle_namespace_elements(&path, namespace);    
                }
                Decl::FuncDecl(func) => {
                    if let Some(current_namespace) = current_namespace {
                        let name = Ident::new(
                            self.db,
                            func.name
                                .get_text(self.file.document(self.db).texter.text.as_bytes())
                                .unwrap()
                                .to_string(),
                        );
                        self.paths
                            .get_mut(&current_namespace)
                            .unwrap()
                            .functions
                            .insert(name, func.parse(self.db, self.file));
                    }
                }
                Decl::FbDecl(fb) => {
                    if let Some(current_namespace) = current_namespace {
                        let name = Ident::new(
                            self.db,
                            fb.name
                                .get_text(self.file.document(self.db).texter.text.as_bytes())
                                .unwrap()
                                .to_string(),
                        );
                        self.paths
                            .get_mut(&current_namespace)
                            .unwrap()
                            .function_blocks
                            .insert(name, fb.parse(self.db, self.file));
                    }
                }
                Decl::ClassDecl(class) => {
                    if let Some(current_namespace) = current_namespace {
                        let name = Ident::new(
                            self.db,
                            class
                                .name
                                .get_text(self.file.document(self.db).texter.text.as_bytes())
                                .unwrap()
                                .to_string(),
                        );
                        self.paths
                            .get_mut(&current_namespace)
                            .unwrap()
                            .classes
                            .insert(name, class.parse(self.db, self.file));
                    }
                }
                _ => (),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{solver::{namespace_path, namespaces_in_file}, RootDatabase};
    use auto_lsp::{
        default::db::{FileManager},
        lsp_types,
        texter::core::text::Text,
    };
    
    use salsa::EventKind;

    use super::*;

    #[test]
    fn multiple_namespaces() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let texter = Text::new(
            r#"
NAMESPACE TEST.k
    NAMESPACE TEST235333.m.a
        NAMESPACE TEST.b
            FUNCTION N

            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE
END_NAMESPACE"#
                .into(),
        );
        db.add_file_from_texter(
            ast::RK_PARSER.get("structured_text").unwrap(),
            &url,
            texter,
        )
        .unwrap();

        let file = db.get_file(&url).unwrap();
        let namespaces = namespaces_in_file(&db, file);

        let actual = namespaces
                .namespaces
                .iter()
                .map(|n| n.0.display(&db))
                .collect::<Vec<_>>();

        assert_eq!(actual.len(), 3);    
        assert!(actual.contains(&"TEST.k".to_string()));
        assert!(actual.contains(&"TEST.k.TEST235333.m.a".to_string()));
        assert!(actual.contains(&"TEST.k.TEST235333.m.a.TEST.b".to_string()));
    }

    #[test]
    fn interned_paths() {
        let db = RootDatabase::default();
        let first_id = Ident::new(&db, "first".to_string());
        let second_id = Ident::new(&db, "second".to_string());

        assert_ne!(first_id, second_id);

        let first_path = NamespacePath::from((&db as _, vec![Ident::new(&db, "first".to_string()), Ident::new(&db, "second".to_string())]));
        let second_path = NamespacePath::from((&db as _, vec![Ident::new(&db, "first".to_string()), Ident::new(&db, "second".to_string())]));

        assert_eq!(first_path, second_path);
    }

    #[test]
    fn tracked_namespaces() {
        let logs = Arc::new(Mutex::new(Vec::new()));
        let ptr = logs.clone();

        let mut db = RootDatabase::new(Some(Box::new(move |event| {
            if let EventKind::WillExecute{ .. } = event.kind  {
                ptr.lock().unwrap().push(event);
            }
        })));

        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let texter = Text::new(
            r#"
NAMESPACE first
    NAMESPACE second
        NAMESPACE third
            FUNCTION N

            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE
END_NAMESPACE"#
                .into(),
        );
        db.add_file_from_texter(
            ast::RK_PARSER.get("structured_text").unwrap(),
            &url,
            texter,
        )
        .unwrap();

        let first = NamespacePath::from((&db as _, &vec![Ident::new(&db, "first".to_string())]));
        let second = NamespacePath::from((&db as _, &vec![Ident::new(&db, "first".to_string()), Ident::new(&db, "second".to_string())]));
        let third = NamespacePath::from((&db as _, &vec![Ident::new(&db, "first".to_string()), Ident::new(&db, "second".to_string()), Ident::new(&db, "third".to_string())]));
        
        assert!(!namespace_path(&db, first).is_empty());
        assert!(!namespace_path(&db, second).is_empty());
        assert!(!namespace_path(&db, third).is_empty());

        logs.lock().unwrap().clear();

        // Getting paths on a same file should not trigger recomputation

        assert!(!namespace_path(&db, first).is_empty());
        assert!(!namespace_path(&db, second).is_empty());
        assert!(!namespace_path(&db, third).is_empty());

        assert_eq!(logs.lock().unwrap().len(), 0);
    }
}
