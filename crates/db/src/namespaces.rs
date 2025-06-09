use std::collections::{HashMap};
use std::ops::Deref;
use std::sync::Arc;

use ast::generated::{ClassDecl_DataTypeDecl_FbDecl_FuncDecl_InterfaceDecl_NamespaceDecl, NamespaceDecl};
use auto_lsp::core::document::Document;
use auto_lsp::default::db::{tracked::get_ast, BaseDatabase, File};
use auto_lsp::salsa;
use auto_lsp::core::ast::AstNode;

#[salsa::interned(debug, no_lifetime)]
pub struct Ident {
    #[return_ref]
    pub text: String,
}

#[salsa::interned(debug, no_lifetime)]
pub struct NamespacePath {
    #[return_ref]
    pub path: Vec<Ident>,
}

impl NamespacePath {
    pub fn concat(&self, db: &dyn BaseDatabase, other: &NamespacePath) -> NamespacePath {
        let mut path = self.path(db).to_vec();
        path.extend(other.path(db));
        NamespacePath::new(db, &path)
    }

    pub fn extend(&self, db: &dyn BaseDatabase, ident: Ident) -> NamespacePath {
        let mut path = self.path(db).to_vec();
        path.push(ident);
        NamespacePath::new(db, &path)
    }

    pub fn display(&self, db: &dyn BaseDatabase) -> String {
        self.path(db).iter().map(|i| i.text(db)).collect::<Vec<_>>().join(".")
    }
}

#[derive(Default, Clone, Debug, PartialEq, salsa::Update)]
pub struct LocalNamespaces(Arc<HashMap<NamespacePath, NamespaceSlice>>);

impl Deref for LocalNamespaces {
    type Target = HashMap<NamespacePath, NamespaceSlice>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default, Clone, Debug, PartialEq, salsa::Update)]
pub struct NamespaceSlice {
    pub internal: bool, 
    pub scopes: HashMap<Ident, Scope>,
}

impl NamespaceSlice {
    pub fn new(namespace: &NamespaceDecl, scopes: HashMap<Ident, Scope>) -> Self {
        Self {
            internal: namespace.internal.is_some(),
            scopes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    Class,
    DataType,
    Fb,
    Func,
    Interface,
}

#[salsa::tracked(no_eq)]
pub fn namespaces_in_file(db: &dyn BaseDatabase, file: File) -> LocalNamespaces {
    let ast = get_ast(db, file).get_root().unwrap();
    let source = ast.downcast_ref::<ast::generated::SourceFile>().unwrap();

    LocalNamespacesBuilder::new(db, file, source).build()
}

#[salsa::tracked(returns(ref))]
pub fn namespace_path<'db>(db: &'db dyn BaseDatabase, path: NamespacePath) -> Vec<LocalNamespaces> {
    db.get_files().iter().enumerate().filter_map(|(i, file)| {
        let namespaces = namespaces_in_file(db, *file);
        if namespaces.0.contains_key(&path) {
            Some(namespaces)
        } else {
            None
        }
    }).collect()
}

pub struct LocalNamespacesBuilder<'db> {
    db: &'db dyn BaseDatabase,
    file: File,
    source: &'db ast::generated::SourceFile,
    paths: HashMap<NamespacePath, NamespaceSlice>,
}

impl<'db> LocalNamespacesBuilder<'db> {
    pub fn new(db: &'db dyn BaseDatabase, file: File, source: &'db ast::generated::SourceFile) -> Self {
        Self { db, file, source, paths: HashMap::default() }
    }

    pub fn build(mut self) -> LocalNamespaces {
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
                    self.paths.entry(namespace_path).or_insert(NamespaceSlice::new(namespace, HashMap::default()));

                    if let Some(elements) = &namespace.elements {
                        self.build_nested_namespace(&path, elements);
                    }
                },
                _ => (),
            }
        });
        LocalNamespaces(Arc::new(self.paths))
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
                    self.paths.entry(namespace_path).or_insert(NamespaceSlice::new(namespace, HashMap::default()));
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

fn add_path(db: &dyn BaseDatabase, doc: &Document, path: &mut Vec<Ident>, namespace: &ast::generated::NamespaceDecl) -> Vec<Ident> {
    let mut n_path = namespace.name.children.iter().map(|n| {
        let text = doc.texter.text.as_bytes();
        Ident::new(db, n.get_text(text).unwrap().to_string())
    }).collect::<Vec<_>>(); 
    n_path.extend(path.iter().cloned());
    n_path
}


pub fn namespace_solver(db: &dyn BaseDatabase, file: File, node: &dyn AstNode) -> NamespacePath {
    let list = get_ast(db, file);
    let mut path = vec![];

    if let Some(namespace) = node.downcast_ref::<ast::generated::NamespaceDecl>() {
        path = add_path(db, &file.document(db), &mut path, namespace);
    }

    let mut node = node.get_parent(list);
    while let Some(parent) = node {
        if let Some(parent) = parent.lower().downcast_ref::<ast::generated::NamespaceDecl>() {
            path = add_path(db, &file.document(db), &mut path, parent);
        }
        node = parent.get_parent(list);
    }

    NamespacePath::new(db, &path)
}

#[cfg(test)]
mod tests {
    use auto_lsp::{default::db::{BaseDb, FileManager}, lsp_types, texter::core::text::Text};
    use expect_test::expect;
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
