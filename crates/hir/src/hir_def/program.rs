use std::sync::{Arc, LazyLock};

use auto_lsp::{default::db::{BaseDatabase}, lsp_types::Url};
use db::WorkspaceDataBase;

use crate::{AstId, HirNodeInfo, hir_def::{
    config::AccessDirection, expressions::{expression::PathExpr, spec::Spec, statement::Stmt}, interned::identifier::Ident, pous::variable::{DirectVariable, VariableDecl}, scope::ScopeId, semantic_index::semantic_index
}};

pub static MAIN_FILE_URL: LazyLock<Arc<Url>> = LazyLock::new(|| Arc::new(Url::parse("file:///main.st").unwrap()));


#[salsa::tracked(returns(as_ref))]
pub fn get_programs<'db>(
    db: &'db dyn WorkspaceDataBase,
) -> Option<Vec<ProgramDecl<'db>>> {
    db.get_file(&MAIN_FILE_URL)
        .map(|file| {
            let semantic_index = semantic_index(db, file);
            Some(semantic_index.programs.clone())
        })
        .unwrap_or_default()
}

#[salsa::tracked(debug)]
pub struct ProgramDecl<'db> {
    name: Ident,

    #[returns(ref)]
    prog_access_decls: Vec<ProgAccessDecl<'db>>,

    #[returns(ref)]
    variables: Vec<VariableDecl<'db>>,

    #[returns(ref)]
    statements: Vec<Stmt<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for ProgramDecl<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

#[derive(Debug, Clone, Hash, salsa::Update)]
pub struct ProgAccessDecl<'db> {
    pub name: Ident,

    pub variable: PathExpr<'db>,

    pub direct_variable: Option<DirectVariable>,

    pub spec: Spec<'db>,

    pub direction: Option<AccessDirection>
}
