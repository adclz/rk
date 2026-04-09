use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::Parse;
use crate::hir_def::expressions::expression::{ParamAssign, ParamAssignKind};
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::pous::warn_pragma::{WarnPragma, WarnPragmaLevel};
use auto_lsp::core::ast::AstNode;

/// Extracted pragma data from a unified `pou_pragma` list.
pub struct ParsedPouPragmas<'db> {
    pub is_test: bool,
    pub is_once: bool,
    pub warn_pragma: Option<WarnPragma>,
    pub cases: Vec<Vec<ParamAssign<'db>>>,
}

impl<'db> SemanticIndexBuilder<'db> {
    /// Parse the unified `pragmas` list from a POU declaration.
    pub fn parse_pou_pragmas(
        &mut self,
        pragmas: &[auto_lsp::core::ast::AstNodeId<ast::generated::PouPragma>],
    ) -> ParsedPouPragmas<'db> {
        use ast::generated::CasePragma_OncePragma_TestPragma_WarnPragma as PragmaKind;

        let mut result = ParsedPouPragmas {
            is_test: false,
            is_once: false,
            warn_pragma: None,
            cases: vec![],
        };

        for pragma_id in pragmas {
            let pragma = pragma_id.cast(self.ast);
            match pragma.children.cast(self.ast) {
                PragmaKind::TestPragma(_) => {
                    result.is_test = true;
                }
                PragmaKind::OncePragma(_) => {
                    result.is_once = true;
                }
                PragmaKind::WarnPragma(warn) => {
                    result.warn_pragma = self.parse_warn_pragma(&warn);
                }
                PragmaKind::CasePragma(case) => {
                    result.cases.push(self.parse_single_case(&case));
                }
            }
        }

        result
    }

    fn parse_warn_pragma(&self, warn: &ast::generated::WarnPragma) -> Option<WarnPragma> {
        let doc = self.file.document(self.db).as_bytes();

        let level = match warn.level.cast(self.ast).children.cast(self.ast) {
            ast::generated::Info_Warn::Warn(_) => WarnPragmaLevel::Warn,
            ast::generated::Info_Warn::Info(_) => WarnPragmaLevel::Info,
        };

        let msg_text = warn.message.cast(self.ast).get_text(doc).ok()?;
        let message = compact_str::CompactString::from(&msg_text[1..msg_text.len() - 1]);

        Some(WarnPragma { level, message })
    }

    fn parse_single_case(
        &mut self,
        case: &ast::generated::CasePragma,
    ) -> Vec<ParamAssign<'db>> {
        let mut args = vec![];
        for arg_id in case.args.iter() {
            match arg_id.cast(self.ast) {
                ast::generated::Comma_ParamAssignInput::ParamAssignInput(p) => {
                    let kind = match p.param.as_ref() {
                        Some(param) => {
                            let param = SpanIdent::from_node(self.db, self, param.cast(self.ast));
                            let value = p.value.cast(self.ast).parse(self);
                            match (param, value) {
                                (Ok(param), Ok(value)) => {
                                    ParamAssignKind::FormalInput { param, value }
                                }
                                _ => continue,
                            }
                        }
                        None => match p.value.cast(self.ast).parse(self) {
                            Ok(value) => ParamAssignKind::NonFormal { value },
                            _ => continue,
                        },
                    };
                    args.push(self.new_param(p.into(), self.current_scope, kind));
                }
                _ => {} // skip commas
            }
        }
        args
    }
}
