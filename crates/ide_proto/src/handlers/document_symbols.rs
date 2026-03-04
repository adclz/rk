use auto_lsp::{core::document_symbols_builder::DocumentSymbolsBuilder, lsp_types::SymbolKind};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        config::{ConfigDecl, ProgConfig, ResourceDecl},
        expressions::spec::{ElementarySpec, SpecKind},
        hir_node::HirNode,
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        program::ProgramDecl,
    },
    hir_ty::head::{inheritance::MethodRef, signature::infer_signature},
};

use crate::handlers::DocumentSymbolsHandler;

impl<'db> DocumentSymbolsHandler<'db> for HirNode<'db> {
    fn document_symbols(
        &self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    ) {
        match self {
            HirNode::Program(p) => p.document_symbols(db, builder),
            HirNode::Namespace(n) => n.document_symbols(db, builder),
            HirNode::PouDecl(p) => p.document_symbols(db, builder),
            HirNode::VariableDecl(v) => v.document_symbols(db, builder),
            HirNode::MethodRef(m) => m.document_symbols(db, builder),
            HirNode::Config(c) => c.document_symbols(db, builder),
            HirNode::Resource(r) => r.document_symbols(db, builder),
            HirNode::ProgConfig(p) => p.document_symbols(db, builder),
            _ => (),
        }
    }
}

impl<'db> DocumentSymbolsHandler<'db> for ProgramDecl<'db> {
    fn document_symbols(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    ) {
        let mut nested_builder = DocumentSymbolsBuilder::default();

        let name = self.name(db).text(db).to_string();
        let name = match name.len() {
            0 => "?".into(),
            _ => name,
        };

        self.variables(db)
            .iter()
            .for_each(|var| var.document_symbols(db, &mut nested_builder));

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some("PROGRAM".to_string()),
            kind: auto_lsp::lsp_types::SymbolKind::MODULE, // Program
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.get_name_span(db).lsp(),
            children: Some(nested_builder.finalize()),
            tags: None,
        });
    }
}

impl<'db> DocumentSymbolsHandler<'db> for NamespaceDecl<'db> {
    fn document_symbols(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    ) {
        let mut nested_builder = DocumentSymbolsBuilder::default();
        self.namespaces(db)
            .iter()
            .for_each(|ns| ns.document_symbols(db, &mut nested_builder));

        self.pous(db)
            .iter()
            .for_each(|pou| pou.document_symbols(db, &mut nested_builder));

        let name = self.path(db).to_string(db);
        let name = match name.len() {
            0 => "?".into(),
            _ => name,
        };

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some("NAMESPACE".to_string()),
            kind: auto_lsp::lsp_types::SymbolKind::NAMESPACE, // Namespace
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.name_span(db).lsp(),
            children: Some(nested_builder.finalize()),
            tags: None,
        });
    }
}

impl<'db> DocumentSymbolsHandler<'db> for Pou<'db> {
    fn document_symbols(
        &'db self,
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

        let infer = infer_signature(db, self.get_scope_id(db));

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some(match self {
                Pou::FunctionBlock(_) => "FUNCTION_BLOCK".to_string(),
                Pou::Function(_) => "FUNCTION".to_string(),
                Pou::Class(_) => "CLASS".to_string(),
                Pou::DataType(dt) => infer.type_of_specs[&dt.spec(db)].type_name(db),
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
                    SpecKind::SizedString(_) | SpecKind::SizedWString(_) => SymbolKind::STRING,
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
}

impl<'db> DocumentSymbolsHandler<'db> for VariableDecl<'db> {
    fn document_symbols(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    ) {
        let name = self.name(db).text(db).to_string();
        let name = match name.len() {
            0 => "?".into(),
            _ => name,
        };

        let infer = infer_signature(db, self.get_scope_id(db));

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some(infer.type_of_specs[&self.spec(db)].type_name(db)),
            kind: SymbolKind::VARIABLE,
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.get_name_span(db).lsp(),
            children: None,
            tags: None,
        });
    }
}

impl<'db> DocumentSymbolsHandler<'db> for MethodRef<'db> {
    fn document_symbols(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    ) {
        let name = self.get_name_ident(db).text(db).to_string();
        let name = match name.len() {
            0 => "?".into(),
            _ => name,
        };

        let mut nested_builder = DocumentSymbolsBuilder::default();
        self.variables(db)
            .iter()
            .for_each(|var| var.document_symbols(db, &mut nested_builder));

        let infer = infer_signature(db, self.get_scope_id(db));

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some(format!(
                "METHOD{}",
                match self.return_type(db) {
                    Some(dt) => format!(" : {}", infer.type_of_specs[dt].type_name(db)),
                    None => "".into(),
                }
            )),
            kind: SymbolKind::METHOD,
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.get_name_span(db).lsp(),
            children: Some(nested_builder.finalize()),
            tags: None,
        });
    }
}

impl<'db> DocumentSymbolsHandler<'db> for ConfigDecl<'db> {
    fn document_symbols(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    ) {
        let mut nested_builder = DocumentSymbolsBuilder::default();

        self.variables(db)
            .iter()
            .for_each(|var| var.document_symbols(db, &mut nested_builder));

        for res in self.resources(db) {
            match res {
                hir::hir_def::config::ConfigResource::Resource(r) => {
                    r.document_symbols(db, &mut nested_builder)
                }
                hir::hir_def::config::ConfigResource::Program(p) => {
                    p.document_symbols(db, &mut nested_builder)
                }
                _ => {}
            }
        }

        let name = self.name(db).text(db).to_string();

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some("CONFIGURATION".to_string()),
            kind: SymbolKind::MODULE,
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.get_name_span(db).lsp(),
            children: Some(nested_builder.finalize()),
            tags: None,
        });
    }
}

impl<'db> DocumentSymbolsHandler<'db> for ResourceDecl<'db> {
    fn document_symbols(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    ) {
        let mut nested_builder = DocumentSymbolsBuilder::default();

        self.variables(db)
            .iter()
            .for_each(|var| var.document_symbols(db, &mut nested_builder));

        self.programs(db)
            .iter()
            .for_each(|p| p.document_symbols(db, &mut nested_builder));

        let name = self.name(db).ident.text(db).to_string();

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some(format!(
                "RESOURCE ON {}",
                self.resource_type_name(db).text(db)
            )),
            kind: SymbolKind::MODULE,
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.name(db).get_span(db).lsp(),
            children: Some(nested_builder.finalize()),
            tags: None,
        });
    }
}

impl<'db> DocumentSymbolsHandler<'db> for ProgConfig<'db> {
    fn document_symbols(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        builder: &mut DocumentSymbolsBuilder,
    ) {
        let name = self.name(db).ident.text(db).to_string();

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some("PROGRAM".to_string()),
            kind: SymbolKind::MODULE,
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.name(db).get_span(db).lsp(),
            children: None,
            tags: None,
        });
    }
}
