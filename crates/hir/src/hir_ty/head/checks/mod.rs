use db::WorkspaceDataBase;

use crate::{
    HasName,
    check::errors::{ToIdeDiagnostic, e2_resolve::ResolveError, e3_type::TypeError},
    hir_def::{
        expressions::spec::{Spec, SpecKind},
        interned::namespace::SpanNamespaceAccess,
        pous::generics::{GenericParam, derive_generic_params},
        scope::ScopeKind,
        semantic_index::get_scope,
    },
    hir_ty::{
        head::init_inference::InitInference,
        resolver::name::{NameResolution, resolve_name},
        ty::Type,
    },
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
            SpecKind::Target(target) => {
                self.check_target_generic_args(db, spec, target);
            }
            _ => {}
        }
    }

    /// Validate that a `user_type_ref` supplying `<...>` (or a bare reference
    /// to a generic POU) matches the referenced FB/Class's implicit parameter
    /// list — the ordered, deduplicated set of `ANY_*` specs on its top-level
    /// variables. Emits an E032x diagnostic on any mismatch.
    fn check_target_generic_args(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        spec: Spec<'db>,
        target: &SpanNamespaceAccess<'db>,
    ) {
        // Resolve the path to a POU; ignore non-resolution errors (E02xx
        // resolver already reports those).
        let pou = match resolve_name(db, &target.path, spec.scope_id(db)) {
            NameResolution::Pou(p, _) => p,
            _ => return,
        };
        let params: Vec<GenericParam> = derive_generic_params(db, &pou);
        let args = &target.type_args;
        let name = pou.get_name_ident(db).text(db).to_string();

        match (params.is_empty(), args.is_empty()) {
            // Non-generic POU + no args supplied — nothing to check.
            (true, true) => {}
            // Non-generic POU + user wrote `<...>` — reject.
            (true, false) => {
                self.errors
                    .push(TypeError::GenericArgsOnNonGenericType { name, spec }.to_diagnostic(db, self.scope.file(db)));
            }
            // Generic POU + user wrote nothing — require explicit args.
            (false, true) => {
                self.errors.push(
                    TypeError::MissingGenericArgs {
                        name,
                        expected: params.len(),
                        spec,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
            // Generic POU + user wrote args — check count then bounds.
            (false, false) => {
                if params.len() != args.len() {
                    self.errors.push(
                        TypeError::WrongNumberOfGenericArgs {
                            name,
                            expected: params.len(),
                            actual: args.len(),
                            spec,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                    return;
                }
                for (param, arg_spec) in params.iter().zip(args.iter()) {
                    let arg_ty = Type::resolve_spec(db, *arg_spec);
                    if !matches!(&arg_ty, Type::Elementary(e) if param.bound.accepts(*e)) {
                        self.errors.push(
                            TypeError::TypeArgDoesNotMatchBound {
                                arg_ty,
                                bound: param.bound,
                                arg_spec: *arg_spec,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                }
            }
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
                    .push(ResolveError::IntoRefNotAny { spec, ident, ty }.to_diagnostic(db, self.scope.file(db)));
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
                            ResolveError::IntoRefNotAny { spec, ident, ty }.to_diagnostic(db, self.scope.file(db)),
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
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
            return;
        }

        // Not found
        self.errors
            .push(ResolveError::IntoRefNotFound { spec, ident }.to_diagnostic(db, self.scope.file(db)));
    }
}
