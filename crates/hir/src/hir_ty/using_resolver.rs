use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, HirNodeInfo,
    hir_def::{
        namespace::NamespaceDecl,
        scope::ScopeId,
        using::Using,
    }, hir_ty::name_res::global_namespace_index,
};

#[salsa::tracked(debug)]
pub struct ResolvedUsing<'db> {
    pub using: Using<'db>,

    #[returns(ref)]
    pub namespaces: Vec<NamespaceDecl<'db>>,
}

impl<'db> HirNodeInfo<'db> for ResolvedUsing<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.using(db).get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.using(db).get_scope_id(db)
    }
}

#[tracing::instrument(skip_all, name = "using_resolver")]
#[salsa::tracked(returns(ref), no_eq)]
pub fn resolve_using<'db>(db: &'db dyn BaseDatabase, using: Using<'db>) -> ResolvedUsing<'db> {
    let path = using.path(db);

    let matching_namespaces = global_namespace_index(db).get(&path);
    ResolvedUsing::new(db, using, matching_namespaces.map(|n| {
        n.clone()
    }).unwrap_or_default())
}
