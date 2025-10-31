use hir::{
    hir_def::{
        expressions::spec::{ElementarySpec, SpecKind}, interned::namespace::SpanNamespaceAccess, pous::{
            class::MethodDecl,
            function_block::FunctionBlock,
            pou::{Pou, PouDecl},
            variable::{VariableDecl, VariableKind},
        }
    }, hir_ty::{
        implementation::find_all_implementations,
        inheritance_solver::{declared_methods, MethodRef},
    }, TypeInfo
};

use auto_lsp::{
    core::document_symbols_builder::DocumentSymbolsBuilder,
    default::db::BaseDatabase,
    lsp_types::{
        CodeLens, Command, CompletionItem, CompletionItemKind, CompletionItemLabelDetails,
        GotoDefinitionResponse, Hover, HoverContents, InlayHint, InlayHintKind, InlayHintLabel,
        Location, LocationLink, MarkupContent, MarkupKind, SymbolKind,
        request::GotoImplementationResponse,
    },
};
use hir::HirNodeInfo;
use serde_json::to_value;

use crate::{HasComment, ToProtocol, completions};

impl<'db> ToProtocol<'db> for PouDecl<'db> {
    fn document_symbols(&self, db: &'db dyn BaseDatabase, builder: &mut DocumentSymbolsBuilder) {
        let mut nested_builder = DocumentSymbolsBuilder::default();
        match self.pou(db) {
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

        let name = self.name(db).text(db).to_string();
        let name = match name.len() {
            0 => "?".into(),
            _ => name,
        };

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some(match self.pou(db) {
                Pou::FunctionBlock(_) => "FUNCTION_BLOCK".to_string(),
                Pou::Function(_) => "FUNCTION".to_string(),
                Pou::Class(_) => "CLASS".to_string(),
                Pou::DataType(dt) => dt.spec(db).to_ty(db).type_name(db),
                Pou::Interface(_) => "INTERFACE".to_string(),
            }),
            kind: match self.pou(db) {
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
                        | ElementarySpec::Dt
                        | ElementarySpec::Ldt
                        | ElementarySpec::Date
                        | ElementarySpec::LDate => SymbolKind::EVENT,
                    },
                    _ => SymbolKind::TYPE_PARAMETER,
                },
                Pou::Interface(_) => SymbolKind::INTERFACE,
            },
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.get_name_span(db).unwrap().lsp(),
            children: Some(nested_builder.finalize()),
            tags: None,
        });
    }

    fn inlay_hint(&'db self, db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        Some(InlayHint {
            label: InlayHintLabel::String(format!(
                "{} {}",
                match self.pou(db) {
                    Pou::Function(_) => "FUNCTION",
                    Pou::FunctionBlock(_) => "FUNCTION_BLOCK",
                    Pou::Class(_) => "CLASS",
                    Pou::Interface(_) => "INTERFACE",
                    Pou::DataType(_) => None?,
                },
                self.name(db).text(db)
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

    fn implementation(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoImplementationResponse> {
        match self.pou(db) {
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

    fn code_lens(&self, db: &'db dyn BaseDatabase) -> Option<CodeLens> {
        match self.pou(db) {
            Pou::Class(_) | Pou::Interface(_) => {
                let implementations = find_all_implementations(db, *self);
                if implementations.is_empty() {
                    None
                } else {
                    Some(CodeLens {
                        range: self.get_span(db).lsp(),
                        command: Some(Command {
                            title: format!("{} implementations", implementations.len()),
                            command: "rk.showImplementations".into(),
                            arguments: Some(vec![
                                to_value(self.get_scope_id(db).file(db).url(db).as_str()).unwrap(),
                                to_value(self.name_span(db).lsp().start).unwrap(),
                            ]),
                        }),
                        data: None,
                    })
                }
            }
            _ => None,
        }
    }

    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        let name_span = self.name_span(db);

        // Return None if the offset is outside the name span
        if offset < name_span.start_byte || offset >= name_span.end_byte {
            return None;
        }

        let comment = self.get_comment(db).unwrap_or_default();
        let kind = match self.pou(db) {
            Pou::Function(_) => "FUNCTION".into(),
            Pou::FunctionBlock(_) => "FUNCTION_BLOCK".into(),
            Pou::Class(_) => "CLASS".into(),
            Pou::Interface(_) => "INTERFACE".into(),
            Pou::DataType(dt) => dt.spec(db).to_ty(db).type_name(db),
        };

        let name = self.name(db).text(db);
        let return_type = match self.pou(db) {
            Pou::Function(f) => f
                .return_type(db)
                .map(|spec| format!(": {}", spec.to_ty(db).type_name(db)))
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

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        Some(GotoDefinitionResponse::Scalar(Location::new(
            self.scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        match self.pou(db) {
            Pou::Function(f) => {
                return Some(f.completion(db, offset));
            }
            Pou::FunctionBlock(fb) => {
                return Some(fb.completion(db, offset));
            }
            Pou::DataType(dt) => {
                return dt.spec(db).completion(db, offset);
            }
            _ => {}
        }

        let mut results = vec![];
        self.scope_id(db).global_variables(db).iter().for_each(|(name, v)| {
            results.push(CompletionItem {
                label: name.text(db).to_string(),
                label_details: Some(CompletionItemLabelDetails {
                    detail: Some(
                        match v.kind(db) {
                            VariableKind::Input => "(INPUT)",
                            VariableKind::Output => "(OUTPUT)",
                            VariableKind::InOut => "(IN_OUT",
                            VariableKind::Var => "(VAR)",
                            VariableKind::External => "(EXTERNAL)",
                            VariableKind::Global => "(GLOBAL)",
                            VariableKind::Access => "(ACCESS)",
                            VariableKind::Config => "(CONFIG)",
                            VariableKind::Temp => "(TEMP)",
                        }
                        .into(),
                    ),
                    ..Default::default()
                }),
                detail: Some(v.spec(db).to_ty(db).type_name(db)),
                kind: Some(CompletionItemKind::VARIABLE),
                ..CompletionItem::default()
            })
        });

        declared_methods(db, *self)
            .iter()
            .for_each(|(name, method)| {
                results.push(CompletionItem {
                    label: method.name(db).text(db).to_string(),
                    detail: match method.return_type(db) {
                        Some(ret_type) => Some(format!(
                            "{}: {}",
                            name.text(db),
                            ret_type.to_ty(db).type_name(db)
                        )),
                        None => None,
                    },
                    kind: Some(CompletionItemKind::METHOD),
                    ..CompletionItem::default()
                })
            });

        Some(results)
    }
}

pub enum CursorLocation {
    BeforeExtends,
    BeforeImplements,
    BeforeVariables,
    AfterVariables,
    BeforeMethods,
    AfterMethods,
}

pub trait PrecizeCompletion<'db> {
    fn completion(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Vec<CompletionItem>;

    fn location(
        &self,
        db: &'db dyn BaseDatabase,
        extends: Option<&'db SpanNamespaceAccess<'db>>,
        implements: Option<&'db [SpanNamespaceAccess<'db>]>,
        variables: Option<&'db [VariableDecl<'db>]>,
        methods: Option<&'db [MethodDecl<'db>]>,
        offset: usize,
    ) -> CursorLocation {
        if let Some(extends) = extends {
            if offset < extends.get_span(db).start_byte {
                return CursorLocation::BeforeExtends;
            }
        }

        if let Some(implements) = implements {
            if let Some(first_impl) = implements.first() {
                if offset < first_impl.get_span(db).start_byte {
                    return CursorLocation::BeforeImplements;
                }
            }
        }


        if let Some(variables) = variables {
            if let Some(first_var) = variables.first() {
                if offset < first_var.get_span(db).start_byte {
                    return CursorLocation::BeforeVariables;
                }
            }

            if let Some(last_var) = variables.last() {
                if offset > last_var.get_span(db).end_byte {
                    if let Some(methods) = methods {
                        if let Some(first_method) = methods.first() {
                            if offset < first_method.get_span(db).start_byte {
                                return CursorLocation::AfterVariables;
                            }
                        } else {
                            return CursorLocation::AfterVariables;
                        }
                    } else {
                        return CursorLocation::AfterVariables;
                    }
                }
            }
        }

        if let Some(methods) = methods {
            if let Some(first_method) = methods.first() {
                if offset < first_method.get_span(db).start_byte {
                    return CursorLocation::BeforeMethods;
                }
            }
        }

        CursorLocation::AfterMethods
    }
}
