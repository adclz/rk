use auto_lsp::default::db::BaseDatabase;

use crate::{hir::{expression::Expr, variable::Variable, visibility::Modifiers}, to_proto::{IterToProto, SymbolInfo, ToProto}};


#[salsa::tracked(debug)]
pub struct FunctionBlock<'db> {
    pub extends: Option<Expr<'db>>,

    pub implements: Option<Vec<Expr<'db>>>,

    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,

    pub modifiers: Modifiers,
}


impl<'db> IterToProto<'db> for FunctionBlock<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.variables(db).iter().map(|v| v as _)
    }
}