use std::ops::Deref;

use crate::{
    hir::{
        interned::identifier::Ident,
        pous::{
            data_type::DataType,
            pou::{Pou, PouDecl},
        },
        scopes::scope::{FileScopeId, Scope, ScopeKind, Visibility},
    },
    parser::{semantic_index::SemanticIndexBuilder, ParseInit, ParseSpec},
};
use ast::generated::TypeDecl;
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_data_type(&mut self, data_type: &TypeDecl) -> anyhow::Result<PouDecl<'db>> {
        let scope_id = FileScopeId::from((self.file, data_type.get_id()));
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let name = Ident::from_node(self.db, self.file, data_type.name.deref())?;

        type Spec = ast::generated::ArrayTypeSpec_EnumTypeSpec_RefTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec_SubrangeTypeSpec;

        let spec = match data_type.spec.deref() {
            Spec::ArrayTypeSpec(a) => a.to_spec(self),
            Spec::EnumTypeSpec(a) => a.to_spec(self),
            Spec::SimpleTypeSpec(a) => a.to_spec(self),
            Spec::StrTypeSpec(a) => a.to_spec(self),
            Spec::StructTypeSpec(a) => a.to_spec(self),
            Spec::SubrangeTypeSpec(a) => a.to_spec(self),
            Spec::RefTypeSpec(a) => a.to_spec(self),
        }?;

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

        let init = match data_type.init.as_deref() {
            Some(Init::ArrayTypeInit(a)) => Some(a.to_init(self)?),
            Some(Init::SimpleTypeInit(a)) => Some(a.to_init(self)?),
            Some(Init::StructTypeInit(a)) => Some(a.to_init(self)?),
            None => None,
        };

        let scope_id = self.create_pou_id(data_type);

        let result = PouDecl::new(
            self.db,
            Pou::DataType(DataType::new(self.db, spec, init, scope_id)),
            data_type.get_span(),
            name,
            data_type.name.get_span(),
            scope_id,
        );

        let scope = Scope::new(
            self.file,
            ScopeKind::Pou(result),
            vec![],
            scope_id,
            Visibility::empty(),
            Some(previous_scope),
        );

        self.scope_keys.insert(scope_id, scope);

        Ok(result)
    }
}
