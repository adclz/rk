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

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_warn_pragma(
        &self,
        warn: &ast::generated::WarnPragma,
    ) -> Option<crate::hir_def::pous::warn_pragma::WarnPragma> {
        use auto_lsp::core::ast::AstNode;
        use crate::hir_def::pous::warn_pragma::{WarnPragma, WarnPragmaLevel};

        let doc = self.file.document(self.db).as_bytes();

        let level = match warn.level.cast(self.ast).children.cast(self.ast) {
            ast::generated::Info_Warn::Warn(_) => WarnPragmaLevel::Warn,
            ast::generated::Info_Warn::Info(_) => WarnPragmaLevel::Info,
        };

        let msg_text = warn.message.cast(self.ast).get_text(doc).ok()?;
        let message = compact_str::CompactString::from(&msg_text[1..msg_text.len() - 1]);

        Some(WarnPragma { level, message })
    }
}
