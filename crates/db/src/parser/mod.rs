use auto_lsp::anyhow;

use crate::{
    hir::{expressions::expression::Expr, expressions::spec::Spec, pous::variable::Variable},
    parser::semantic_index::SemanticIndexBuilder,
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
        sema: &SemanticIndexBuilder<'db>,
        section: &mut Vec<Variable<'db>>,
    ) -> anyhow::Result<()>;
}

pub trait ParseSpec<'db> {
    fn to_spec(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Spec<'db>>;
}

pub trait ParseInit<'db> {
    fn to_init(&self, sema: &SemanticIndexBuilder<'db>) -> anyhow::Result<Expr<'db>>;
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
    fn to_spec_init(&self, sema: &SemanticIndexBuilder<'db>)
        -> anyhow::Result<SpecInitResult<'db>>;
}

impl<'db, T: ParseSpec<'db> + ParseInit<'db>> ParseSpecInit<'db> for T {
    fn to_spec_init(
        &self,
        sema: &SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>> {
        Ok(SpecInitResult::new(
            self.to_spec(sema)?,
            Some(self.to_init(sema)?),
        ))
    }
}
