use std::fmt::format;

use crate::{
    completions,
    hir_def::{
        expressions::spec::{ElementarySpec, SpecKind},
        modifier::Modifier,
    },
    hir_ty::implementation::find_all_implementations,
};
use auto_lsp::{
    core::document_symbols_builder::DocumentSymbolsBuilder,
    default::db::BaseDatabase,
    lsp_types::{
        CodeLens, Command, CompletionItem, InlayHint, InlayHintKind, InlayHintLabel, LocationLink,
        SymbolKind, request::GotoImplementationResponse,
    },
};
use serde_json::to_value;

use crate::{
    hir_def::{
        interned::identifier::Ident,
        pous::{
            class::Class, data_type::DataType, function::Function, function_block::FunctionBlock,
            interface::Interface,
        },
        scope::FileScopeId,
    },
    to_proto::{AstId, ToProto},
};

#[salsa::tracked(debug)]
pub struct PouDecl<'db> {
    #[tracked]
    #[returns(ref)]
    pub pou: Pou<'db>,

    #[returns(ref)]
    pub name: Ident,

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> PouDecl<'db> {
    pub fn modifier(&'db self, db: &'db dyn BaseDatabase) -> Modifier {
        match self.pou(db) {
            Pou::Class(class) => class.modifier(db),
            Pou::FunctionBlock(fb) => fb.modifier(db),
            _ => Modifier::empty(),
        }
    }
}

impl<'db> ToProto<'db> for PouDecl<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }

    fn document_symbols(&self, db: &'db dyn BaseDatabase, builder: &mut DocumentSymbolsBuilder) {
        let mut nested_builder = DocumentSymbolsBuilder::default();
        match self.pou(db) {
            Pou::FunctionBlock(fb) => {
                fb.variables(db)
                    .iter()
                    .for_each(|var| var.document_symbols(db, &mut nested_builder));
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
                    .for_each(|m| m.document_symbols(db, &mut nested_builder));
            }
            Pou::Interface(i) => {
                i.methods(db)
                    .iter()
                    .for_each(|m| m.document_symbols(db, &mut nested_builder));
            }
            _ => {}
        }

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name: self.name(db).text(db).to_string(),
            detail: Some(
                match self.pou(db) {
                    Pou::FunctionBlock(_) => "function block",
                    Pou::Function(_) => "function",
                    Pou::Class(_) => "class",
                    Pou::DataType(_) => "data type",
                    Pou::Interface(_) => "interface",
                }
                .to_string(),
            ),
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

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        match self.pou(db) {
            Pou::Function(f) => f.completion_ctx(db, offset),
            Pou::FunctionBlock(_) => Some(vec![completions::snippets::var_input()]),
            Pou::Class(_) => Some(vec![completions::snippets::var_input()]),
            Pou::Interface(_) => Some(vec![completions::snippets::var_input()]),
            Pou::DataType(_) => None,
        }
    }

    fn inlay_hint(&'db self, db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        Some(InlayHint {
            label: InlayHintLabel::String(format!(
                "{} {}",
                match self.pou(db) {
                    Pou::Function(_) => "function",
                    Pou::FunctionBlock(_) => "function block",
                    Pou::Class(_) => "class",
                    Pou::Interface(_) => "interface",
                    Pou::DataType(_) => "data type",
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
                                to_value(self.get_name_span(db).unwrap().lsp().start).unwrap()
                            ]),
                        }),
                        data: None,
                    })
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update, salsa::Supertype)]
pub enum Pou<'db> {
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),
    DataType(DataType<'db>),
}
