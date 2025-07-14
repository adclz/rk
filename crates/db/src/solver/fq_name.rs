use std::ops::Deref;

use super::namespace::NamespacePath;
use crate::{
    hir::namespace::{Namespace, PouDecl},
    ident::Ident,
    solver::namespace::namespace_path,
};
use auto_lsp::{anyhow, {core::span::Span}, default::db::{file::File, BaseDatabase}};
use auto_lsp::core::ast::AstNode;

#[derive(Clone, Hash, salsa::Update, Debug)]
pub struct SpannedNamespaceAccess {
    pub span: Span,
    pub fq_name: NamespaceAccess,
}

impl SpannedNamespaceAccess {
    pub fn new(db: &dyn BaseDatabase, file: File, fq_name: &ast::generated::NamespaceAccess) -> anyhow::Result<Self> {
        Ok(SpannedNamespaceAccess { span: fq_name.get_span(), fq_name: NamespaceAccess::from_ast(db, file, &fq_name)? })
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        self.fq_name.to_string(db)
    }
}

/// Interned Fully Qualified Name
#[salsa::interned(debug, no_lifetime)]
pub struct NamespaceAccess {
    pub namespace: Option<NamespacePath>,
    pub target: Ident,
}

impl NamespaceAccess {
    pub fn from_ast(db: &dyn BaseDatabase, file: File, fq_name: &ast::generated::NamespaceAccess) -> anyhow::Result<Self> {
    let mut fragments = Vec::new();

    // Walk the tree from root down `.path` fields
    let mut current = match fq_name.children.deref() {
        ast::generated::Identifier_ScopedIdentifier::ScopedIdentifier(scoped) => scoped,
        ast::generated::Identifier_ScopedIdentifier::Identifier(ident) => {
            let target = Ident::from_node(db, file, ident)?;
            return Ok(NamespaceAccess::new(db, None, target));
        }
    };

    loop {
        // Extract the target of the current scoped_identifier (e.g. m1, m2, m3...)
        fragments.push(Ident::from_node(db, file, current.target.deref())?);

        match current.path.deref() {
            ast::generated::Identifier_ScopedIdentifier::ScopedIdentifier(next) => {
                current = next;
            }
            ast::generated::Identifier_ScopedIdentifier::Identifier(base) => {
                // Reached the bottom-most path (e.g. "system")
                fragments.push(Ident::from_node(db, file, base)?);
                break;
            }
        }
    }

    fragments.reverse(); // Because we walked right-to-left

    // The last identifier pushed is the *rightmost*, so pop it off to become the target
    let target = fragments.pop().expect("There must be at least one identifier");

    Ok(NamespaceAccess::new(
        db,
        Some(NamespacePath::from((db, fragments))),
        target,
    ))
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        let mut path = self.namespace(db)
            .map(|ns| ns.to_string(db))
            .unwrap_or_else(String::default);
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(self.target(db).text(db).as_str());
        path
    }
}

#[derive(Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum FqSolverResult<'db> {
    Hidden(Namespace<'db>),
    Ok(PouDecl<'db>),
    None,
}

impl Default for FqSolverResult<'_> {
    fn default() -> Self {
        FqSolverResult::None
    }
}

/// Returns the pou declaration for the given fq name
///
/// If there are multiple declarations, returns the first one
#[salsa::tracked(returns(ref))]
pub fn fq_name_solver<'db>(
    db: &'db dyn BaseDatabase,
    fq: NamespaceAccess,
    file: File,
) -> FqSolverResult<'db> {
    let path = if let Some(fq) = fq.namespace(db) {
        fq
    } else {
        return FqSolverResult::None;
    };
    let target = fq.target(db);
    
    for n in namespace_path(db, path) {
        let Some(ns) = n.namespaces(db).get(&path) else {
            continue;
        };

        let Some(pou) = ns.get_pou(db, target) else {
            continue;
        };

        if ns.internal(db) {
            if n.file(db) == file {
                return FqSolverResult::Ok(*pou);
            } else {
                return FqSolverResult::Hidden(*ns);
            }
        } else {
            return FqSolverResult::Ok(*pou);
        }
    }

    FqSolverResult::None
}

#[cfg(test)]
mod tests {
    use auto_lsp::{default::db::FileManager, lsp_types};

    use super::*;
    use crate::{hir::namespace::{Pou, PouResult}, ident::Ident, solver::namespace::{namespaces_in_file, NamespacePath}, RootDatabase};

    #[test]
    fn parse_fq_name() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE ns
    FUNCTION_BLOCK f IMPLEMENTS System::Subsystem::Target
    END_FUNCTION_BLOCK
END_NAMESPACE
"#;
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

        let ns = NamespacePath::from((&db as _, vec![Ident::new(&db, "ns".to_string())]));
        let function = namespaces.get_pou(
            &db,
            ns,
            ns,
            Ident::new(&db, "f".to_string()),
        );

        let PouResult::Found(pou) = function else {
            panic!("Not a function block");
        };

        if let Pou::FunctionBlock(f) = pou.pou(&db) {
            assert!(f.implements(&db).is_some());

            let implements = &f.implements(&db).unwrap()[0];
            assert_eq!(implements.fq_name.namespace(&db).unwrap().fragments(&db).len(), 2);
            assert_eq!(implements.fq_name.target(&db).text(&db), "Target");

        } else {
            panic!("Not a function block");
        }

    }

    #[test]
    fn pou_solver() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE ns
    FUNCTION f

    END_FUNCTION
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

        let fq = NamespaceAccess::new(
            &db,
            Some(NamespacePath::from((&db as _, vec![Ident::new(&db, "ns".to_string())]))),
            Ident::new(&db, "f".to_string()),
        );
        let pou = fq_name_solver(&db, fq, file);
        if let FqSolverResult::Ok(pou) = pou {
            assert_eq!(*pou.name(&db), Ident::new(&db, "f".to_string()));
        } else {
            panic!("Not a pou");
        };
    }

    #[test]
    fn hidden_pou_solver() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = r#"
NAMESPACE INTERNAL ns
    FUNCTION f

    END_FUNCTION
END_NAMESPACE"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();

        let url = lsp_types::Url::parse("file:///test2.st").unwrap();
        let source = r#"
NAMESPACE ns2
    FUNCTION f

    END_FUNCTION
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

        let fq = NamespaceAccess::new(
            &db,
            Some(NamespacePath::from((&db as _, vec![Ident::new(&db, "ns".to_string())]))),
            Ident::new(&db, "f".to_string()),
        );
        let pou = fq_name_solver(&db, fq, file);
        if let FqSolverResult::Hidden(ns) = pou {
            assert_eq!(
                *ns.path(&db),
                NamespacePath::from((&db as _, vec![Ident::new(&db, "ns".to_string())]))
            );
        } else {
            panic!("Not a hidden pou");
        };
    }
}
