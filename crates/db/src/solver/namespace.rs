use auto_lsp::{core::span::Span, default::db::{file::File, tracked::get_ast, BaseDatabase}};

use crate::{
    hir::namespace::{FileNamespaces, Namespace, NamespaceResult, Using},
    ident::{Ident, SpannedIdent},
    parser::namespace::FileNamespacesBuilder,
};

/// Interned namespace path
#[salsa::interned(debug, no_lifetime)]
pub struct NamespacePath {
    #[returns(ref)]
    pub fragments: Vec<SpannedIdent>,
}

impl<'db> NamespacePath {
    pub fn concat(&self, db: &dyn BaseDatabase, other: &NamespacePath) -> NamespacePath {
        let mut path = self.fragments(db).to_owned();
        path.extend_from_slice(&other.fragments(db));
        NamespacePath::new(db, path)
    }

    pub fn extend(&self, db: &dyn BaseDatabase, ident: SpannedIdent) -> NamespacePath {
        let mut path = self.fragments(db).to_owned();
        path.push(ident);
        NamespacePath::new(db, path)
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        self.fragments(db)
            .iter()
            .map(|i| i.ident.text(db))
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl From<(&dyn BaseDatabase, &SpannedIdent)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &SpannedIdent)) -> Self {
        NamespacePath::new(from.0, vec![from.1.clone()])
    }
}

impl From<(&dyn BaseDatabase, &[SpannedIdent])> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &[SpannedIdent])) -> Self {
        NamespacePath::new(from.0, from.1.to_vec())
    }
}

impl From<(&dyn BaseDatabase, Vec<SpannedIdent>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, Vec<SpannedIdent>)) -> Self {
        NamespacePath::new(from.0, from.1)
    }
}

impl From<(&dyn BaseDatabase, &Vec<SpannedIdent>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &Vec<SpannedIdent>)) -> Self {
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
            if namespaces.namespaces(db).iter().find(|ns| ns.path(db) == &path).is_some() {
                Some(namespaces)
            } else {
                None
            }
        })
        .collect()
}

/// Namespaces accessible in a given file and namespace path
pub fn namespaces_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    namespace: Namespace) -> Vec<NamespacePath> {
    let span = namespace.span(db);
    let mut results = vec![];

    let namespaces = match namespaces_in_file(db, file) {
        Some(namespaces) => namespaces,
        None => return results,
    };

    for ns in namespaces.namespaces(db) {
        let other_span = ns.span(db);

        // Checks if the span intersects with the given namespace span
        if span.start_byte <= other_span.end_byte
            && span.end_byte >= other_span.start_byte
        {
            // Checks if a prent namespace matches
            if ns.path(db) == namespace.path(db) {
                results.push(*ns.path(db));
            }

            // Checks if Using directives already imports the same namespace
            let using_directives = ns.using(db);
            results.extend(using_directives.iter().map(|using| using.path(db)).collect::<Vec<_>>());
        }
    }

    results
    
}


/// Namespaces accessible in a given file and namespace path
pub fn using_namespaces_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    namespace: Using) -> Vec<(&'db Span, NamespacePath)> {
    let span = namespace.span(db);
    let mut results = vec![];

    let namespaces = match namespaces_in_file(db, file) {
        Some(namespaces) => namespaces,
        None => return results,
    };

    match namespace.parent_id(db) {
        Some(parent) => {
            let mut parent = parent;
            while let Some(ns) = namespaces.namespace_keys(db).get(&parent) {
                eprintln!("Checking namespace: {:?} with {:?}", ns.path(db).to_string(db), namespace.path(db).to_string(db));
                
                // Checks if the namespace itself matches
                if ns.path(db) == &namespace.path(db) {
                    results.push((ns.span(db), *ns.path(db)));
                }


                // Checks if sibling namespaces match
                for sibling in ns.child_namespaces_keys(db) {
                    let sibling = namespaces.namespace_keys(db).get(sibling).unwrap();
                    if sibling.path(db) == &namespace.path(db) && sibling != ns {
                        results.push((sibling.span(db), *sibling.path(db)));
                    }
                }

                // Check if the using directives match
                for using in ns.using(db) {
                    if using.path(db) == namespace.path(db) && using != &namespace {
                        results.push((using.span(db), using.path(db)));
                    }
                }
                // Move to the parent namespace
                parent = match ns.parent(db) {
                    Some(parent_id) => parent_id,
                    None => break, // No more parent, we reached the root namespace
                };
            }

        }
        None => {
            eprintln!("Searching for root namespace");
            // If no parent, we are looking for the root namespace
            if let Some(ns) = namespaces.namespaces(db).iter().find(|ns| ns.path(db).fragments(db).is_empty()) {
                results.push((ns.span(db), *ns.path(db)));
            }
        }
    }

    results
    
}

pub fn starts_with<'db>(db: &'db dyn BaseDatabase, ident: Ident) -> Vec<Namespace<'db>> {
    db.get_files()
        .iter()
        .filter_map(|file| namespaces_in_file(db, *file))
        .flat_map(|ns| {
            ns.namespaces(db).iter().find_map(|ns| {
                if ns.path(db).fragments(db)[0].ident.text(db).starts_with(&ident.text(db)) {
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
            ns.namespaces(db).iter().filter_map(|ns| {
                if ns.path(db).fragments(db)[0] == ident {
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
    fn intern_namespace() {
        let db = RootDatabase::default();

        let first = NamespacePath::from((&db as _, &vec![SpannedIdent::from_blank(&db, "first")]));
        let second = NamespacePath::from((
            &db as _,
            &vec![
                SpannedIdent::from_blank(&db, "first"),
                SpannedIdent::from_blank(&db, "second"),
            ],
        ));
        let third = NamespacePath::from((
            &db as _,
            &vec![
                SpannedIdent::from_blank(&db, "first"),
                SpannedIdent::from_blank(&db, "second"),
                SpannedIdent::from_blank(&db, "third"),
            ],
        ));

        assert!(first.fragments(&db).len() == 1);
        assert!(second.fragments(&db).len() == 2);
        assert!(third.fragments(&db).len() == 3);
    }

    #[test]
    fn interned_namespace_paths() {
        let db = RootDatabase::default();
        let first_id = SpannedIdent::from_blank(&db, "first");
        let second_id = SpannedIdent::from_blank(&db, "second");

        assert_ne!(first_id, second_id);

        let first_path = NamespacePath::from((
            &db as _,
            vec![
                SpannedIdent::from_blank(&db, "first"),
                SpannedIdent::from_blank(&db, "second"),
            ],
        ));
        let second_path = NamespacePath::from((
            &db as _,
            vec![
                SpannedIdent::from_blank(&db, "first"),
                SpannedIdent::from_blank(&db, "second"),
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

        let test = namespaces.namespaces(&db).iter().next().unwrap();
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

        let test = namespaces.namespaces(&db).iter().next().unwrap();

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
                n.path(&db).fragments(&db)
                    .iter()
                    .map(|i| i.ident.text(&db))
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

        let first = NamespacePath::from((&db as _, &vec![SpannedIdent::from_blank(&db, "first")]));
        let second = NamespacePath::from((
            &db as _,
            &vec![
                SpannedIdent::from_blank(&db, "first"),
                SpannedIdent::from_blank(&db, "second"),
            ],
        ));
        let third = NamespacePath::from((
            &db as _,
            &vec![
                SpannedIdent::from_blank(&db, "first"),
                SpannedIdent::from_blank(&db, "second"),
                SpannedIdent::from_blank(&db, "third"),
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
            NamespacePath::from((&db as _, &vec![SpannedIdent::from_blank(&db, "first")])),
        );

        assert_eq!(all_namespaces.len(), 1);
        assert_eq!(logs.lock().unwrap().len(), 3);

        logs.lock().unwrap().clear();

        // Getting all namespaces again should not trigger recomputation until the next db revision

        let all_namespaces = namespace_path(
            &db,
            NamespacePath::from((&db as _, &vec![SpannedIdent::from_blank(&db, "first")])),
        );
        assert_eq!(all_namespaces.len(), 1);
        assert_eq!(logs.lock().unwrap().len(), 0);
    }


    /*#[test]
    fn in_scope() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE ns
    NAMESPACE ns2
        FUNCTION f  
        END_FUNCTION
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
        let ns = namespaces_in_file(db, file).unwrap().namespaces(db);

        let file = db.get_file(&url).unwrap();
        let scopes = namespaces_in_scope(&db, file, 
            NamespacePath::from((&db as _, &vec![
                SpannedIdent::from_blank(&db, "ns"),
                SpannedIdent::from_blank(&db, "ns2")
            ]))
        );
        assert_eq!(scopes.len(), 2);
    }*/
}
