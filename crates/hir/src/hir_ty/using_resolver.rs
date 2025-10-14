use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{hir_def::{interned::{identifier::SpanIdent, namespace::NamespacePath}, namespace::NamespaceDecl, scope::FileScopeId, using::Using}, hir_ty::name_res::shared_namespaces, AstId, HirNodeInfo};

#[salsa::tracked(debug)]
pub struct ResolvedUsing<'db> {
    pub using: Using<'db>,

    #[returns(ref)]
    pub namespaces: Vec<NamespaceDecl<'db>>,
}

impl<'db> HirNodeInfo<'db> for ResolvedUsing<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.using(db).get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.using(db).get_scope_id(db)
    }
}


#[tracing::instrument(skip_all, name = "using_resolver")]
#[salsa::tracked(returns(ref), no_eq)]
pub fn resolve_using<'db>(
    db: &'db dyn BaseDatabase,
    using: Using<'db>,
) -> ResolvedUsing<'db> {
    let path = using.path(db);

    let matching_namespaces = shared_namespaces(db, path);
    ResolvedUsing::new(db, using, matching_namespaces.to_vec())
}