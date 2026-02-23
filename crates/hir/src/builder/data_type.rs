use std::sync::Arc;

use crate::Visibility;
use crate::{
    builder::{ParseSpec, expression::ParseExpr, semantic_index::SemanticIndexBuilder},
    check::errors::analysis_error::AnalysisError,
    hir_def::{
        interned::identifier::Ident,
        pous::{data_type::DataType, pou::Pou},
        scope::{Scope, ScopeKind},
    },
};
use ast::generated::TypeDecl;
use auto_lsp::anyhow;

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_data_type(
        &mut self,
        data_type: &TypeDecl,
    ) -> anyhow::Result<Pou<'db>, AnalysisError<'db>> {
        let scope_id = self.generate_scope_id();
        let previous_scope = self.current_scope;
        self.current_scope = scope_id;

        let name = Ident::from_node(self.db, self.file, data_type.name.cast(self.ast))?;

        type Spec = ast::generated::ArrayTypeSpec_EnumTypeSpec_RefTypeSpec_SimpleTypeSpec_StructTypeSpec_SubrangeTypeSpec;

        let spec = match data_type.spec.cast(self.ast) {
            Spec::ArrayTypeSpec(a) => a.to_spec(self),
            Spec::EnumTypeSpec(a) => a.to_spec(self),
            Spec::SimpleTypeSpec(a) => a.to_spec(self),
            Spec::StructTypeSpec(a) => a.to_spec(self),
            Spec::SubrangeTypeSpec(a) => a.to_spec(self),
            Spec::RefTypeSpec(a) => a.to_spec(self),
        }?;

        type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

        let init = match data_type.init.as_ref() {
            Some(init_node) => match init_node.cast(self.ast) {
                Init::ArrayTypeInit(a) => Some(a.parse(self)?),
                Init::SimpleTypeInit(a) => Some(a.parse(self)?),
                Init::StructTypeInit(a) => Some(a.parse(self)?),
            },
            None => None,
        };

        let result = Pou::DataType(DataType::new(
            self.db,
            name,
            data_type.name.cast(self.ast).into(),
            spec,
            init,
            data_type.into(),
            scope_id,
        ));

        let scope = Scope::new(
            self.file,
            ScopeKind::Pou(result),
            vec![],
            scope_id,
            Visibility::empty(),
            Some(previous_scope),
        );

        self.scope_keys
            .insert(scope_id.scope(self.db), Arc::new(scope));

        Ok(result)
    }
}
