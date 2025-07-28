use auto_lsp::{anyhow, default::db::file::File};

use crate::{
    hir::{
        expressions::expression::Expr,
        pous::variable::{Spec, Variable},
    },
    BaseDatabase,
};

pub mod class;
pub mod data_type;
pub mod expression;
pub mod function;
pub mod function_block;
pub mod interface;
pub mod namespace;
pub mod semantic_index;
pub mod statement;
pub mod types;
pub mod using;
pub mod variables;

pub trait ParseVarSection<'db> {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()>;
}

pub trait ParseSpec<'db> {
    fn to_spec(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Spec<'db>>;
}

pub trait ParseInit<'db> {
    fn to_init(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Expr<'db>>;
}

pub struct SpecInitResult<'db> {
    pub spec: Spec<'db>,
    pub init: Option<Expr<'db>>,
}

impl<'db> SpecInitResult<'db> {
    pub fn new(spec: Spec<'db>, init: Option<Expr<'db>>) -> Self {
        Self { spec, init }
    }
}

pub trait ParseSpecInit<'db> {
    fn to_spec_init(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<SpecInitResult<'db>>;
}

impl<'db, T: ParseSpec<'db> + ParseInit<'db>> ParseSpecInit<'db> for T {
    fn to_spec_init(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        Ok(SpecInitResult::new(
            self.to_spec(db, file)?,
            Some(self.to_init(db, file)?),
        ))
    }
}
