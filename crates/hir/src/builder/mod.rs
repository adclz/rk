use auto_lsp::anyhow;

use crate::{
    builder::semantic_index::SemanticIndexBuilder,
    check::errors::analysis_error::AnalysisError,
    hir_def::{
        expressions::{expression::InitExpr, spec::Spec},
        pous::variable::VariableDecl,
    },
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
pub mod invocation;

pub trait ParseVarSection<'db> {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>);
}

pub trait ParseSpec<'db> {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, AnalysisError<'db>>;
}

pub trait ParseInit<'db> {
    fn to_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<InitExpr<'db>, AnalysisError<'db>>;
}

pub struct SpecInitResult<'db> {
    pub spec: Spec<'db>,
    pub init: Option<InitExpr<'db>>,
}

impl<'db> SpecInitResult<'db> {
    pub fn new(spec: Spec<'db>, init: Option<InitExpr<'db>>) -> Self {
        Self { spec, init }
    }
}

pub trait ParseSpecInit<'db> {
    fn to_spec_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>, AnalysisError<'db>>;
}

impl<'db, T: ParseSpec<'db> + ParseInit<'db>> ParseSpecInit<'db> for T {
    fn to_spec_init(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<SpecInitResult<'db>, AnalysisError<'db>> {
        Ok(SpecInitResult::new(
            self.to_spec(sema)?,
            Some(self.to_init(sema)?),
        ))
    }
}
