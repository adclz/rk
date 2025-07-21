use auto_lsp::{core::span::Span, default::db::BaseDatabase};

use crate::{
    hir::{
        namespace::Using,
        variable::{Spec, Variable},
    },
    ident::Ident,
    solver::fq_name::SpannedPath,
    to_proto::{HirCtx, IterToProto, ProtoAndCtx, SymbolInfo, ToProto},
};

#[salsa::tracked(debug)]
pub struct Interface<'db> {
    #[returns(as_ref)]
    pub extends: Option<Vec<SpannedPath>>,

    #[tracked]
    #[returns(ref)]
    pub using: Vec<Using<'db>>,

    #[returns(ref)]
    pub methods: Vec<Method<'db>>,
}

impl<'db> IterToProto<'db> for Interface<'db> {
    fn iter(&'db self, ctx: HirCtx<'db>) -> impl Iterator<Item = ProtoAndCtx<'db>> {
        self.methods(ctx.db)
            .iter()
            .map(move |m| m.iter(ctx).map(|n| n))
            .flatten()
    }
}

#[salsa::tracked(debug)]
pub struct Method<'db> {
    #[returns(ref)]
    pub range: Span,

    pub name: Ident,

    #[returns(ref)]
    pub name_span: Span,

    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,
}

impl<'db> ToProto<'db> for Method<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.range(db)
    }

    fn get_named_span(&'db self, db: &'db dyn BaseDatabase) -> Option<&'db Span> {
        Some(self.name_span(db))
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(
            SymbolInfo::builder()
                .kind(auto_lsp::lsp_types::SymbolKind::METHOD)
                .name(self.name(db).text(db))
                .range(self.range(db).clone())
                .name_range(self.name_span(db).clone())
                .maybe_spec(self.return_type(db).cloned())
                .build(),
        )
    }
}

impl<'db> IterToProto<'db> for Method<'db> {
    fn iter(&'db self, ctx: HirCtx<'db>) -> impl Iterator<Item = ProtoAndCtx<'db>> {
        Box::new(self.variables(ctx.db).iter().map(move |v| (ctx, v as _)))
    }
}
