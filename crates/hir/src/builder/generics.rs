use auto_lsp::anyhow;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    builder::{ParseSpec, semantic_index::SemanticIndexBuilder},
    hir_def::{
        interned::identifier::{Ident, SpanIdent},
        pous::generics::{GenericContraint, GenericParam, SpecContraint},
    },
};

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_generic_params(
        &mut self,
        generic_spec: &ast::generated::GenericSpec,
    ) -> anyhow::Result<Vec<GenericParam<'db>>, IdeDiagnostic> {
        let mut result = vec![];

        for generic in generic_spec.children.cast(self.ast).children.iter() {
            let generic = generic.cast(self.ast);

            let r = Ident::from_node(self.db, self.file, generic.generic_name.cast(self.ast));
            let Some(name) = self.try_parse(r) else { continue };
            let name_id = generic.generic_name.cast(self.ast).into();

            let r = SpanIdent::from_node(self.db, self, generic.generic_type.cast(self.ast));
            let Some(value) = self.try_parse(r) else { continue };
            let generic_contraint = GenericContraint {
                value,
                ast_id: generic.generic_type.cast(self.ast).into(),
            };

            let mut spec_constraints = vec![];

            for spec_constraint in generic.constraint.iter() {
                let r = spec_constraint.cast(self.ast).to_spec(self);
                let Some(spec) = self.try_parse(r) else { continue };
                spec_constraints.push(SpecContraint {
                    spec,
                    ast_id: spec_constraint.cast(self.ast).into(),
                });
            }

            result.push(GenericParam::new(
                self.db,
                name,
                name_id,
                generic_contraint,
                spec_constraints,
                self.current_scope,
            ));
        }
        Ok(result)
    }
}
