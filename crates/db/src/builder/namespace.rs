use std::collections::{HashMap};
use std::ops::Deref;
use std::sync::Arc;

use ast::generated::{ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl, NamespaceDecl};
use auto_lsp::core::document::Document;
use auto_lsp::default::db::{tracked::get_ast, BaseDatabase, File};
use auto_lsp::salsa;
use auto_lsp::core::ast::AstNode;

use crate::hir::namespace::{FileNamespaces, Namespace, Scope};
use crate::solver::{Ident, NamespacePath};

pub struct FileNamespacesBuilder<'db> {
    db: &'db dyn BaseDatabase,
    file: File,
    source: &'db ast::generated::SourceFile,
    paths: HashMap<NamespacePath, Namespace>,
}

impl<'db> FileNamespacesBuilder<'db> {
    pub fn new(db: &'db dyn BaseDatabase, file: File, source: &'db ast::generated::SourceFile) -> Self {
        Self { db, file, source, paths: HashMap::default() }
    }

    pub fn build(mut self) -> FileNamespaces {
        // Top level namespaces
        self.source.children.iter().for_each(|child| {
            match child.as_ref() {
                ast::generated::ConfigDecl_NamespaceDecl_ProgDecl::NamespaceDecl(namespace) => {
                    let path = namespace.name.children.iter().map(|n| {
                        let doc = self.file.document(self.db);
                        let text = doc.texter.text.as_bytes();
                        Ident::new(self.db, n.get_text(text).unwrap().to_string())
                    }).collect::<Vec<_>>();
                    let namespace_path = NamespacePath::new(self.db, &path);
                    self.paths.entry(namespace_path).or_insert(Namespace::new(namespace, HashMap::default()));

                    if let Some(elements) = &namespace.elements {
                        self.build_nested_namespace(&path, elements);
                    }
                },
                _ => (),
            }
        });
        FileNamespaces(Arc::new(self.paths))
    }

    pub fn build_nested_namespace(&mut self, parent_path: &[Ident], nested: &ast::generated::NamespaceElements) {
            type Decl = ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl;
            
            let mut current_namespace = None;
            nested.children.iter().for_each(|child| {
                match child.as_ref() {
                    Decl::NamespaceDecl(namespace) => {
                        let mut path = parent_path.to_vec();
                        path.extend(namespace.name.children.iter().map(|n| {
                            let doc = self.file.document(self.db);
                            let text = doc.texter.text.as_bytes();
                            Ident::new(self.db, n.get_text(text).unwrap().to_string())
                        }));   
  
                    let namespace_path: NamespacePath = NamespacePath::new(self.db, &path);
                    self.paths.entry(namespace_path).or_insert(Namespace::new(namespace, HashMap::default()));
                    current_namespace = Some(namespace_path);

                    if let Some(elements) = &namespace.elements {
                        self.build_nested_namespace(&path, elements);
                    }
                },
                Decl::FuncDecl(func) => {
                    if let Some(current_namespace) = current_namespace {
                        let name = Ident::new(self.db, func.name.get_text(self.file.document(self.db).texter.text.as_bytes()).unwrap().to_string());
                        self.paths.get_mut(&current_namespace).unwrap().scopes.insert(name, Scope::Func);
                    }
                }
                _ => ()
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use auto_lsp::{default::db::{BaseDb, FileManager}, lsp_types, texter::core::text::Text};
    use expect_test::expect;
    use crate::solver::namespaces_in_file;

    use super::*;

    #[test]
    fn multiple_namespaces() {
        let mut db = BaseDb::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let texter = Text::new(r#"
NAMESPACE TEST.k
    NAMESPACE TEST235333.m.a
        NAMESPACE TEST.b
            FUNCTION N

            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE
END_NAMESPACE"#.into());
        db.add_file_from_texter(ast::RK_PARSER.get("structured_text").unwrap(), &url, texter).unwrap();

        let file = db.get_file(&url).unwrap();
        let namespaces = namespaces_in_file(&db, file);

        let expected = expect![[r#"[["TEST", "TEST2"], ["TEST"]]"#]];
        let actual = format!("{:?}", namespaces.0.iter().map(|n| n.0.path(&db).iter().map(|n| n.text(&db)).collect::<Vec<_>>()).collect::<Vec<_>>());
        expected.assert_eq(&actual);       
    }

    #[test]
    fn nested_namespaces() {
        let mut db = BaseDb::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let texter = Text::new(r#"
        NAMESPACE TEST

            NAMESPACE TEST2

            END_NAMESPACE

        END_NAMESPACE"#.into());

        db.add_file_from_texter(ast::RK_PARSER.get("structured_text").unwrap(), &url, texter).unwrap();

        let file = db.get_file(&url).unwrap();
        let namespaces = namespaces_in_file(&db, file);

        let expected = expect![[r#"[["TEST", "TEST2"], ["TEST"]]"#]];
        let actual = format!("{:?}", namespaces.0.iter().map(|n| n.0.path(&db)).collect::<Vec<_>>());
        expected.assert_eq(&actual);
    }
}
