use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::pous::pragma::{Pragma, WarnPragma, WarnPragmaLevel};
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    /// Parse the unified `pragmas` list from a POU declaration into `Vec<Pragma>`.
    pub fn parse_pou_pragmas(
        &mut self,
        pragmas: &[auto_lsp::core::ast::AstNodeId<ast::generated::PouPragma>],
    ) -> Vec<Pragma<'db>> {
        use ast::generated::AllowPragma_ExportPragma_ExternPragma_OncePragma_TestPragma_WarnPragma as PragmaKind;

        let mut result = Vec::new();

        for pragma_id in pragmas {
            let pragma = pragma_id.cast(self.ast);
            // Use the PouPragma wrapper node for the span (it wraps the inner pragma)
            let si = match SpanIdent::from_node(self.db, self, pragma) {
                Ok(si) => si,
                Err(_) => continue,
            };
            match pragma.children.cast(self.ast) {
                PragmaKind::TestPragma(_) => {
                    result.push(Pragma::Test(si));
                }
                PragmaKind::OncePragma(_) => {
                    result.push(Pragma::Once(si));
                }
                PragmaKind::ExportPragma(_) => {
                    result.push(Pragma::Export(si));
                }
                PragmaKind::WarnPragma(warn) => {
                    if let Some(wp) = self.parse_warn_pragma(warn) {
                        result.push(Pragma::Warn(si, wp));
                    }
                }
                PragmaKind::ExternPragma(ext) => {
                    if let Some(ep) = self.parse_extern_pragma(ext) {
                        result.push(Pragma::Extern(si, ep));
                    }
                }
                PragmaKind::AllowPragma(allow) => {
                    result.push(Pragma::Allow(si, self.parse_allow_pragma(allow)));
                }
            }
        }

        result
    }

    fn parse_extern_pragma(
        &self,
        ext: &ast::generated::ExternPragma,
    ) -> Option<crate::hir_def::pous::pragma::ExternPragma> {
        let doc = self.file.document(self.db).as_bytes();
        let strip = |t: &str| compact_str::CompactString::from(&t[1..t.len() - 1]);
        let module = strip(ext.module.cast(self.ast).get_text(doc).ok()?);
        let name = strip(ext.name.cast(self.ast).get_text(doc).ok()?);
        Some(crate::hir_def::pous::pragma::ExternPragma { module, name })
    }

    pub(crate) fn parse_allow_pragma(
        &self,
        allow: &ast::generated::AllowPragma,
    ) -> crate::hir_def::pous::pragma::AllowPragma {
        let doc = self.file.document(self.db).as_bytes();
        let rules = allow
            .rule
            .iter()
            .filter_map(|r| {
                let node = r.cast(self.ast);
                let text = node.get_text(doc).ok()?;
                Some((
                    compact_str::CompactString::from(&text[1..text.len() - 1]),
                    node.get_range().to_owned(),
                ))
            })
            .collect();
        crate::hir_def::pous::pragma::AllowPragma { rules }
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
}
