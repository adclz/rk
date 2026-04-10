use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::builder::Parse;
use crate::hir_def::expressions::expression::{ParamAssign, ParamAssignKind};
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::pous::pragma::{Pragma, WarnPragma, WarnPragmaLevel};
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    /// Parse the unified `pragmas` list from a POU declaration into `Vec<Pragma>`.
    pub fn parse_pou_pragmas(
        &mut self,
        pragmas: &[auto_lsp::core::ast::AstNodeId<ast::generated::PouPragma>],
    ) -> Vec<Pragma<'db>> {
        use ast::generated::CasePragma_OncePragma_TestPragma_WarnPragma as PragmaKind;

        let mut result = Vec::new();

        for pragma_id in pragmas {
            let pragma = pragma_id.cast(self.ast);
            match pragma.children.cast(self.ast) {
                PragmaKind::TestPragma(_) => {
                    result.push(Pragma::Test);
                }
                PragmaKind::OncePragma(_) => {
                    result.push(Pragma::Once);
                }
                PragmaKind::WarnPragma(warn) => {
                    if let Some(wp) = self.parse_warn_pragma(&warn) {
                        result.push(Pragma::Warn(wp));
                    }
                }
                PragmaKind::CasePragma(case) => {
                    result.push(Pragma::Case(self.parse_single_case(&case)));
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
                _ => {}
            }
        }
        args
    }
}
