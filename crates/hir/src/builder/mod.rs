use auto_lsp::anyhow;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    builder::semantic_index::SemanticIndexBuilder,
    hir_def::{
        expressions::{expression::InitExpr, spec::Spec},
        pous::variable::VariableDecl,
    },
};

pub mod class;
pub mod config;
pub mod data_type;
pub mod expression;
pub mod function;
pub mod function_block;
pub mod interface;
pub mod namespace;
pub mod pragmas;
pub mod program;
pub mod semantic_index;
pub mod statement;
pub mod types;
pub mod using;
pub mod variables;

pub trait ParseVarSection<'db> {
    fn parse(&self, sema: &mut SemanticIndexBuilder<'db>, section: &mut Vec<VariableDecl<'db>>);
}

pub trait ParseSpec<'db> {
    fn to_spec(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Spec<'db>, IdeDiagnostic>;
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
    ) -> anyhow::Result<SpecInitResult<'db>, IdeDiagnostic>;
}

pub trait Parse<'db> {
    type Output;

    fn parse(
        &self,
        sema: &mut SemanticIndexBuilder<'db>,
    ) -> anyhow::Result<Self::Output, IdeDiagnostic>;
}
