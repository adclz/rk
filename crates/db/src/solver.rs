use auto_lsp::{core::{ast::AstNode, document::Document}, default::db::{tracked::get_ast, BaseDatabase, File}};

use crate::{builder::namespace::FileNamespacesBuilder, hir::namespace::FileNamespaces};

/// Interned identifier
#[salsa::interned(debug, no_lifetime)]
pub struct Ident {
    #[return_ref]
    pub text: String,
}

/// Interned namespace path
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

/// Returns the namespaces in the given file
#[salsa::tracked(no_eq)]
pub fn namespaces_in_file(db: &dyn BaseDatabase, file: File) -> FileNamespaces {
    let ast = get_ast(db, file).get_root().unwrap();
    let source = ast.downcast_ref::<ast::generated::SourceFile>().unwrap();

    FileNamespacesBuilder::new(db, file, source).build()
}

/// Returns the namespaces that contain the given path
#[salsa::tracked(returns(ref))]
pub fn namespace_path<'db>(db: &'db dyn BaseDatabase, path: NamespacePath) -> Vec<FileNamespaces> {
    db.get_files().iter().enumerate().filter_map(|(_, file)| {
        let namespaces = namespaces_in_file(db, *file);
        if namespaces.0.contains_key(&path) {
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

    NamespacePath::new(db, &path)
}
