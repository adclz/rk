use db::WorkspaceDataBase;
use hir::{
    HasName,
    hir_def::{
        expressions::spec::{ElementarySpec, SpecKind},
        pous::pou::Pou,
    },
    hir_ty::{inheritance_solver::MethodRef, ty::Type},
};

use auto_lsp::{
    core::document_symbols_builder::DocumentSymbolsBuilder,
    lsp_types::{
        CodeLens, Command, CompletionItem, GotoDefinitionResponse, Hover, HoverContents, InlayHint,
        InlayHintKind, InlayHintLabel, Location, LocationLink, MarkupContent, MarkupKind,
        SymbolKind, request::GotoImplementationResponse,
    },
};
use hir::HirNodeInfo;
use serde_json::to_value;

use crate::{
    completions::context::ScopeCompletionCtx,
    implementation::find_all_implementations,
    to_proto::{HasComment, ToProtocol},
};

impl<'db> ToProtocol<'db> for Pou<'db> {
    fn document_symbols(
        &self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    ) {
        let mut nested_builder = DocumentSymbolsBuilder::default();
        match self {
            Pou::FunctionBlock(fb) => {
                fb.variables(db)
                    .iter()
                    .for_each(|var| var.document_symbols(db, &mut nested_builder));
                fb.methods(db)
                    .iter()
                    .for_each(|m| MethodRef::from(m).document_symbols(db, &mut nested_builder));
            }
            Pou::Function(f) => {
                f.variables(db)
                    .iter()
                    .for_each(|var| var.document_symbols(db, &mut nested_builder));
            }
            Pou::Class(c) => {
                c.variables(db)
                    .iter()
                    .for_each(|var| var.document_symbols(db, &mut nested_builder));
                c.methods(db)
                    .iter()
                    .for_each(|m| MethodRef::from(m).document_symbols(db, &mut nested_builder));
            }
            Pou::Interface(i) => {
                i.methods(db)
                    .iter()
                    .for_each(|m| MethodRef::from(m).document_symbols(db, &mut nested_builder));
            }
            _ => {}
        }

        let name = self.get_name_ident(db).text(db).to_string();
        let name = match name.len() {
            0 => "?".into(),
            _ => name,
        };

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some(match self {
                Pou::FunctionBlock(_) => "FUNCTION_BLOCK".to_string(),
                Pou::Function(_) => "FUNCTION".to_string(),
                Pou::Class(_) => "CLASS".to_string(),
                Pou::DataType(dt) => Type::new_spec(db, dt.spec(db)).type_name(db),
                Pou::Interface(_) => "INTERFACE".to_string(),
            }),
            kind: match self {
                Pou::FunctionBlock(_) => SymbolKind::FUNCTION,
                Pou::Function(_) => SymbolKind::FUNCTION,
                Pou::Class(_) => SymbolKind::CLASS,
                Pou::DataType(dt) => match dt.spec(db).kind(db) {
                    SpecKind::Enum(_) => SymbolKind::ENUM,
                    SpecKind::Struct(_) => SymbolKind::STRUCT,
                    SpecKind::Array(_) | SpecKind::ArrayConformand(_) | SpecKind::Subrange(_) => {
                        SymbolKind::ARRAY
                    }
                    SpecKind::Simple(simple) => match simple {
                        ElementarySpec::Bool
                        | ElementarySpec::FEDGEBool
                        | ElementarySpec::REDGEBool => SymbolKind::BOOLEAN,
                        ElementarySpec::Byte
                        | ElementarySpec::Word
                        | ElementarySpec::DWord
                        | ElementarySpec::LWord
                        | ElementarySpec::SInt
                        | ElementarySpec::Int
                        | ElementarySpec::DInt
                        | ElementarySpec::LInt
                        | ElementarySpec::USInt
                        | ElementarySpec::UInt
                        | ElementarySpec::UDInt
                        | ElementarySpec::ULInt
                        | ElementarySpec::Real
                        | ElementarySpec::LReal => SymbolKind::NUMBER,
                        ElementarySpec::Char
                        | ElementarySpec::WChar
                        | ElementarySpec::String
                        | ElementarySpec::WString => SymbolKind::STRING,
                        ElementarySpec::Time
                        | ElementarySpec::LTime
                        | ElementarySpec::Tod
                        | ElementarySpec::LTod
                        | ElementarySpec::DateAndTime
                        | ElementarySpec::LDateTime
                        | ElementarySpec::Date
                        | ElementarySpec::LDate => SymbolKind::EVENT,
                    },
                    _ => SymbolKind::TYPE_PARAMETER,
                },
                Pou::Interface(_) => SymbolKind::INTERFACE,
            },
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.get_name_span(db).lsp(),
            children: Some(nested_builder.finalize()),
            tags: None,
        });
    }

    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        Some(InlayHint {
            label: InlayHintLabel::String(format!(
                "{} {}",
                match self {
                    Pou::Function(_) => "FUNCTION",
                    Pou::FunctionBlock(_) => "FUNCTION_BLOCK",
                    Pou::Class(_) => "CLASS",
                    Pou::Interface(_) => "INTERFACE",
                    Pou::DataType(_) => None?,
                },
                self.get_name_ident(db).text(db)
            )),
            position: self.get_span(db).lsp().end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }

    fn implementation(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<GotoImplementationResponse> {
        match self {
            Pou::Class(_) | Pou::Interface(_) => {
                let links = find_all_implementations(db, *self)
                    .iter()
                    .map(|pou| LocationLink {
                        target_uri: pou.get_scope_id(db).file(db).url(db).clone(),
                        target_range: pou.get_span(db).lsp(),
                        target_selection_range: pou.get_span(db).lsp(),
                        origin_selection_range: Some(self.get_span(db).lsp()),
                    })
                    .collect();

                Some(GotoImplementationResponse::Link(links))
            }
            _ => None,
        }
    }

    fn code_lens(&self, db: &'db dyn WorkspaceDataBase) -> Option<CodeLens> {
        match self {
            Pou::Class(_) | Pou::Interface(_) => {
                let implementations = find_all_implementations(db, *self);
                if implementations.is_empty() {
                    None
                } else {
                    Some(CodeLens {
                        range: self.get_span(db).lsp(),
                        command: Some(Command {
                            title: format!(
                                "{} implementation{}",
                                implementations.len(),
                                if implementations.len() > 1 { "s" } else { "" }
                            ),
                            command: "rk.showImplementations".into(),
                            arguments: Some(vec![
                                to_value(self.get_scope_id(db).file(db).url(db).as_str()).unwrap(),
                                to_value(self.get_name_span(db).lsp().start).unwrap(),
                            ]),
                        }),
                        data: None,
                    })
                }
            }
            _ => None,
        }
    }

    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let name_span = self.get_name_span(db);

        // Return None if the offset is outside the name span
        if offset < name_span.start_byte || offset >= name_span.end_byte {
            return None;
        }

        let comment = self.get_comment(db).unwrap_or_default();
        let kind = match self {
            Pou::Function(_) => "FUNCTION".into(),
            Pou::FunctionBlock(_) => "FUNCTION_BLOCK".into(),
            Pou::Class(_) => "CLASS".into(),
            Pou::Interface(_) => "INTERFACE".into(),
            Pou::DataType(dt) => Type::new_spec(db, dt.spec(db)).type_name(db),
        };

        let name = self.get_name_ident(db).text(db);
        let return_type = match self {
            Pou::Function(f) => f
                .return_type(db)
                .map(|spec| format!(": {}", Type::new_spec(db, *spec).type_name(db)))
                .unwrap_or_default(),
            _ => "".to_string(),
        };

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
[{kind}] {name}{return_type}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }

    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        Some(GotoDefinitionResponse::Scalar(Location::new(
            self.get_scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }

    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        Some(
            ScopeCompletionCtx::new(self.get_scope_id(db), offset, "")
                .scoped(db)
                .take_items(),
        )
    }
}
