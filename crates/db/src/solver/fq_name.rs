use super::namespace::NamespacePath;
use crate::{
    diagnostics::{diagnostic_builder::diag, DiagnosticAccumulator},
    hir::namespace::{Namespace, PouDecl},
    ident::Ident,
    solver::namespace::namespace_path,
};
use auto_lsp::default::db::{BaseDatabase, file::File};

/// Interned Fully Qualified Name
#[salsa::interned(debug, no_lifetime)]
pub struct FqName {
    pub namespace: NamespacePath,
    pub target: Ident,
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
    fq: FqName,
    file: File,
) -> FqSolverResult<'db> {
    let path = fq.namespace(db);
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
    use crate::{ident::Ident, solver::namespace::NamespacePath, RootDatabase};

    #[test]
    fn pou_solver() {
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let source = 
            r#"
NAMESPACE ns
    FUNCTION f

    END_FUNCTION
END_NAMESPACE"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call().unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();

        let fq = FqName::new(
            &db,
            NamespacePath::from((&db as _, vec![Ident::new(&db, "ns".to_string())])),
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
        let source = 
            r#"
NAMESPACE INTERNAL ns
    FUNCTION f

    END_FUNCTION
END_NAMESPACE"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call().unwrap();

        db.add_file(file).unwrap();

        let url = lsp_types::Url::parse("file:///test2.st").unwrap();
        let source = 
            r#"
NAMESPACE ns2
    FUNCTION f

    END_FUNCTION
END_NAMESPACE"#;

        let file = File::from_string()
            .db(&db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call().unwrap();

        db.add_file(file).unwrap();

        let file = db.get_file(&url).unwrap();

        let fq = FqName::new(
            &db,
            NamespacePath::from((&db as _, vec![Ident::new(&db, "ns".to_string())])),
            Ident::new(&db, "f".to_string()),
        );
        let pou = fq_name_solver(&db, fq, file);
        if let FqSolverResult::Hidden(ns) = pou {
            assert_eq!(*ns.path(&db), NamespacePath::from((&db as _, vec![Ident::new(&db, "ns".to_string())])));
        } else {
            panic!("Not a hidden pou");
        };
    }
}
