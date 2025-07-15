use auto_lsp::default::db::{file::File, tracked::get_ast, BaseDatabase};

use crate::{
    hir::namespace::{FileNamespaces, Namespace},
    ident::Ident,
    parser::namespace::FileNamespacesBuilder,
};

/// Interned namespace path
#[salsa::interned(debug, no_lifetime)]
pub struct NamespacePath {
    #[returns(ref)]
    pub fragments: Vec<Ident>,
}

impl<'db> NamespacePath {
    pub fn concat(&self, db: &dyn BaseDatabase, other: &NamespacePath) -> NamespacePath {
        let mut path = self.fragments(db).to_owned();
        path.extend_from_slice(&other.fragments(db));
        NamespacePath::new(db, path)
    }

    pub fn extend(&self, db: &dyn BaseDatabase, ident: Ident) -> NamespacePath {
        let mut path = self.fragments(db).to_owned();
        path.push(ident);
        NamespacePath::new(db, path)
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        self.fragments(db)
            .iter()
            .map(|i| i.text(db))
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl From<(&dyn BaseDatabase, &Ident)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &Ident)) -> Self {
        NamespacePath::new(from.0, vec![from.1.clone()])
    }
}

impl From<(&dyn BaseDatabase, &[Ident])> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &[Ident])) -> Self {
        NamespacePath::new(from.0, from.1.to_vec())
    }
}

impl From<(&dyn BaseDatabase, Vec<Ident>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, Vec<Ident>)) -> Self {
        NamespacePath::new(from.0, from.1)
    }
}

impl From<(&dyn BaseDatabase, &Vec<Ident>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &Vec<Ident>)) -> Self {
        NamespacePath::new(from.0, from.1.clone())
    }
}

/// Returns the namespaces in the given file
#[salsa::tracked]
pub fn namespaces_in_file<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
) -> Option<FileNamespaces<'db>> {
    let ast = get_ast(db, file).get_root()?;
    let source = ast.downcast_ref::<ast::generated::SourceFile>()?;

    Some(FileNamespacesBuilder::new(db, file, source).build())
}

/// Returns the namespaces that contain the given path
#[salsa::tracked(returns(ref), no_eq)]
pub fn namespace_path<'db>(
    db: &'db dyn BaseDatabase,
    path: NamespacePath,
) -> Vec<FileNamespaces<'db>> {
    db.get_files()
        .iter()
        .filter_map(|file| {
            let namespaces = match namespaces_in_file(db, *file) {
                Some(namespaces) => namespaces,
                None => return None,
            };
            if namespaces.namespaces(db).contains_key(&path) {
                Some(namespaces)
            } else {
                None
            }
        })
        .collect()
}

pub fn starts_with<'db>(db: &'db dyn BaseDatabase, ident: Ident) -> Vec<Namespace<'db>> {
    db.get_files()
        .iter()
        .filter_map(|file| namespaces_in_file(db, *file))
        .flat_map(|ns| {
            ns.namespaces(db).iter().find_map(|(path, ns)| {
                if path.fragments(db)[0].text(db).starts_with(&ident.text(db)) {
                    Some(*ns)
                } else {
                    None
                }
            })
        })
        .collect()
}

pub fn starts<'db>(db: &'db dyn BaseDatabase, ident: Ident) -> Vec<Namespace<'db>> {
    db.get_files()
        .iter()
        .filter_map(|file| namespaces_in_file(db, *file))
        .flat_map(|ns| {
            ns.namespaces(db).iter().filter_map(|(path, ns)| {
                if path.fragments(db)[0] == ident {
                    Some(*ns)
                } else {
                    None
                }
            })
        })
        .collect()
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
    fn interned_namespace_paths() {
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
    fn namespace_fragments() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE TEST.frag1.frag2.frag3
END_NAMESPACE"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();
        let namespaces = namespaces_in_file(&db, file).unwrap();

        let test = namespaces.namespaces(&db).values().next().unwrap();
        assert!(test.path(&db).fragments(&db).len() == 4);
    }

    #[test]
    fn using_directive_fragments() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE TEST
    USING TEST.frag1.frag2.frag3
END_NAMESPACE"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap());

        let file = file.url(&url).source(source.to_string()).call().unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();
        let namespaces = namespaces_in_file(&db, file).unwrap();

        let test = namespaces.namespaces(&db).values().next().unwrap();

        let using = test.using(&db).first().unwrap();
        assert_eq!(using.path(&db).fragments(&db).len(), 4);
    }

    #[test]
    fn nested_namespaces() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
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
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();
        let namespaces = namespaces_in_file(&db, file);

        let actual = namespaces
            .unwrap()
            .namespaces(&db)
            .iter()
            .map(|n| {
                n.0.fragments(&db)
                    .iter()
                    .map(|i| i.text(&db))
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .collect::<Vec<_>>();

        assert_eq!(actual.len(), 3);
        assert!(actual.contains(&"TEST.k".to_string()));
        assert!(actual.contains(&"TEST.k.TEST235333.m.a".to_string()));
        assert!(actual.contains(&"TEST.k.TEST235333.m.a.TEST.b".to_string()));
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
        let source = r#"
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
            .call()
            .unwrap();

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

        // Getting paths on a same file should not trigger recomputation until the next db revision

        assert!(!namespace_path(&db, first).is_empty());
        assert!(!namespace_path(&db, second).is_empty());
        assert!(!namespace_path(&db, third).is_empty());

        assert_eq!(logs.lock().unwrap().len(), 0);
    }

    #[test]
    fn getting_all_tracked_namespaces() {
        let logs = Arc::new(Mutex::new(Vec::new()));
        let ptr = logs.clone();

        let mut db = RootDatabase::new(Some(Box::new(move |event| {
            if let EventKind::WillExecute { .. } = event.kind {
                ptr.lock().unwrap().push(event);
            }
        })));
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE first

END_NAMESPACE"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let all_namespaces = namespace_path(
            &db,
            NamespacePath::from((&db as _, &vec![Ident::new(&db, "first".to_string())])),
        );

        assert_eq!(all_namespaces.len(), 1);
        assert_eq!(logs.lock().unwrap().len(), 3);

        logs.lock().unwrap().clear();

        // Getting all namespaces again should not trigger recomputation until the next db revision

        let all_namespaces = namespace_path(
            &db,
            NamespacePath::from((&db as _, &vec![Ident::new(&db, "first".to_string())])),
        );
        assert_eq!(all_namespaces.len(), 1);
        assert_eq!(logs.lock().unwrap().len(), 0);
    }
}
