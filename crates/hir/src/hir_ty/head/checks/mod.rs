use db::WorkspaceDataBase;

use crate::{
    HasName,
    check::errors::{ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::{
        expressions::spec::{Spec, SpecKind},
        scope::ScopeKind,
        semantic_index::get_scope,
    },
    hir_ty::{head::init_inference::InitInference, ty::Type},
};

pub mod array;
pub mod enum_;
pub mod methods;
pub mod strukt;
pub mod subrange;
pub mod usings;
pub mod variables;

impl<'db> InitInference<'db> {
    pub fn check_spec(&mut self, db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) {
        match spec.kind(db) {
            SpecKind::Array(arr) => {
                self.check_array(db, *arr);
            }
            SpecKind::Enum(enm) => {
                self.check_enum(db, *enm);
            }
            SpecKind::Subrange(subrange) => {
                self.check_subrange(db, *subrange);
            }
            SpecKind::Struct(strukt) => {
                self.check_struct(db, *strukt);
            }
            SpecKind::Into(span_ident) => {
                self.check_into(db, spec, span_ident);
            }
            _ => {}
        }
    }

    fn check_into(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        spec: Spec<'db>,
        span_ident: &crate::hir_def::interned::identifier::SpanIdent<'db>,
    ) {
        let scope = spec.scope_id(db);
        let ident = span_ident.ident;
        let def_map = scope.def_map(db);

        // Check local variables first
        if let Some(var) = def_map.local_variables.get(&ident) {
            let ty = Type::resolve_spec(db, var.spec(db));
            if !matches!(ty, Type::Elementary(e) if e.is_any()) {
                self.errors
                    .push(ResolveError::IntoRefNotAny { spec, ident, ty }.to_diagnostic(db));
            }
            return;
        }

        // Check if it matches the POU name (function return type)
        if let ScopeKind::Pou(pou) = get_scope(db, scope).kind
            && pou.get_name_ident(db) == ident
        {
            match scope.return_type(db) {
                Some(ret_spec) => {
                    let ty = Type::resolve_spec(db, *ret_spec);
                    if !matches!(ty, Type::Elementary(e) if e.is_any()) {
                        self.errors.push(
                            ResolveError::IntoRefNotAny { spec, ident, ty }.to_diagnostic(db),
                        );
                    }
                }
                None => {
                    // POU has no return type — INTO(fn_name) is invalid
                    self.errors.push(
                        ResolveError::IntoRefNotAny {
                            spec,
                            ident,
                            ty: Type::Never,
                        }
                        .to_diagnostic(db),
                    );
                }
            }
            return;
        }

        // Not found
        self.errors
            .push(ResolveError::IntoRefNotFound { spec, ident }.to_diagnostic(db));
    }
}
