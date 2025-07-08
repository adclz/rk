use auto_lsp::{core::{ast::AstNode, document::Document}, default::db::{tracked::get_ast, BaseDatabase, file::File}};

use crate::{hir::namespace::FileNamespaces, ident::Ident, parser::namespace::FileNamespacesBuilder};

/// Interned namespace path
#[salsa::interned(debug, no_lifetime)]
pub struct NamespacePath {
    pub path: Ident,
}

impl<'db> NamespacePath {
    pub fn concat(&self, db: &dyn BaseDatabase, other: &NamespacePath) -> NamespacePath {
        let mut path = self.path(db).text(db).to_owned();
        path.push_str(&other.path(db).text(db));
        NamespacePath::new(db, Ident::new(db, path))
    }

    pub fn extend(&self, db: &dyn BaseDatabase, ident: Ident) -> NamespacePath {
        let mut path = self.path(db).text(db).to_owned();
        path.push_str(&ident.text(db));
        NamespacePath::new(db, Ident::new(db, path))
    }
}

impl From<(&dyn BaseDatabase, &[Ident])> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &[Ident])) -> Self {
        NamespacePath::new(from.0, Ident::new(from.0, from.1.iter().map(|i| i.text(from.0)).collect::<Vec<_>>().join(".")))
    }
}

impl From<(&dyn BaseDatabase, Vec<Ident>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, Vec<Ident>)) -> Self {
        NamespacePath::new(from.0, Ident::new(from.0, from.1.iter().map(|i| i.text(from.0)).collect::<Vec<_>>().join(".")))
    }
}

impl From<(&dyn BaseDatabase, &Vec<Ident>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &Vec<Ident>)) -> Self {
        NamespacePath::new(from.0, Ident::new(from.0, from.1.iter().map(|i| i.text(from.0)).collect::<Vec<_>>().join(".")))
    }
}

/// Returns the namespaces in the given file
#[salsa::tracked]
pub fn namespaces_in_file<'db>(db: &'db dyn BaseDatabase, file: File) -> Option<FileNamespaces<'db>> {
    let ast = get_ast(db, file).get_root()?;
    let source = ast.downcast_ref::<ast::generated::SourceFile>()?;

    Some(FileNamespacesBuilder::new(db, file, source).build())
}

/// Returns the namespaces that contain the given path
#[salsa::tracked(returns(ref))]
pub fn namespace_path<'db>(db: &'db dyn BaseDatabase, path: NamespacePath) -> Vec<FileNamespaces<'db>> {
    db.get_files().iter().filter_map(|file| {
        let namespaces = match namespaces_in_file(db, *file) {
            Some(namespaces) => namespaces,
            None => return None,
        };
        if namespaces.namespaces(db).contains_key(&path) {
            Some(namespaces)
        } else {
            None
        }
    }).collect()
}

fn add_path(db: &dyn BaseDatabase, doc: &Document, path: &mut Vec<Ident>, namespace: &ast::generated::NamespaceDecl) -> Vec<Ident> {
    let mut n_path = namespace.name.children.iter().map(|n| {
        let text = doc.texter.text.as_bytes();
        Ident::new(db, n.get_text(text).unwrap().to_string())
    }).collect::<Vec<_>>(); 
    n_path.extend(path.iter().cloned());
    n_path
}

/// Returns the namespace path of the given node
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
    NamespacePath::from((db, path))
}


#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        solver::namespace::{namespace_path, namespaces_in_file},
        RootDatabase,
    };
    use auto_lsp::{default::db::FileManager, lsp_types};

    use salsa::EventKind;

    use super::*;

    #[test]
    fn multiple_namespaces() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = 
            r#"
NAMESPACE TEST.k
    NAMESPACE TEST235333.m.a
        NAMESPACE TEST.b
            FUNCTION N

            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE
END_NAMESPACE"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call().unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();
        let namespaces = namespaces_in_file(&db, file);

        let actual = namespaces
            .unwrap()
            .namespaces(&db)
            .iter()
            .map(|n| n.0.path(&db).text(&db))
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

        let first_path = NamespacePath::from((
            &db as _,
            vec![
                Ident::new(&db, "first".to_string()),
                Ident::new(&db, "second".to_string()),
            ],
        ));
        let second_path = NamespacePath::from((
            &db as _,
            vec![
                Ident::new(&db, "first".to_string()),
                Ident::new(&db, "second".to_string()),
            ],
        ));

        assert_eq!(first_path, second_path);
    }

    #[test]
    fn tracked_namespaces() {
        let logs = Arc::new(Mutex::new(Vec::new()));
        let ptr = logs.clone();

        let mut db = RootDatabase::new(Some(Box::new(move |event| {
            if let EventKind::WillExecute { .. } = event.kind {
                ptr.lock().unwrap().push(event);
            }
        })));

        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = 
            r#"
NAMESPACE first
    NAMESPACE second
        NAMESPACE third
            FUNCTION N

            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE
END_NAMESPACE"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call().unwrap();

        db.add_file(file).unwrap();

        let first = NamespacePath::from((&db as _, &vec![Ident::new(&db, "first".to_string())]));
        let second = NamespacePath::from((
            &db as _,
            &vec![
                Ident::new(&db, "first".to_string()),
                Ident::new(&db, "second".to_string()),
            ],
        ));
        let third = NamespacePath::from((
            &db as _,
            &vec![
                Ident::new(&db, "first".to_string()),
                Ident::new(&db, "second".to_string()),
                Ident::new(&db, "third".to_string()),
            ],
        ));

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
