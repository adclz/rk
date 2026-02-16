use auto_lsp::anyhow;

use crate::{
    builder::{ParseSpec, semantic_index::SemanticIndexBuilder},
    check::errors::analysis_error::AnalysisError,
    hir_def::{
        interned::identifier::{Ident, SpanIdent},
        pous::{
            generics::{GenericContraint, GenericParam, SpecContraint},
            variable::VariableDecl,
        },
    },
};

impl<'db> SemanticIndexBuilder<'db> {
    pub fn parse_generic_params(
        &mut self,
        generic_spec: &ast::generated::GenericSpec,
    ) -> anyhow::Result<Vec<GenericParam<'db>>, AnalysisError<'db>> {
        let mut result = vec![];

        for generic in generic_spec.children.cast(self.ast).children.iter() {
            let generic = generic.cast(self.ast);

            let name = Ident::from_node(self.db, self.file, generic.generic_name.cast(self.ast))?;
            let name_id = generic.generic_name.cast(self.ast).into();

            let generic_contraint = GenericContraint {
                value: SpanIdent::from_node(self.db, self, generic.generic_type.cast(self.ast))?,
                ast_id: generic.generic_type.cast(self.ast).into(),
            };

            let mut spec_constraints = vec![];

            for spec_constraint in generic.constraint.iter() {
                spec_constraints.push(SpecContraint {
                    spec: spec_constraint.cast(self.ast).to_spec(self)?,
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
