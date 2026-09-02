use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::HasPragmas;
use crate::check::errors::e2_resolve::ExternForbiddenKind;
use crate::hir_def::{pous::pou::Pou, scope::ScopeKind, semantic_index::get_scope};
use crate::{
    HasName, HirNodeInfo,
    check::errors::{
        ToIdeDiagnostic,
        e1_duplicates::DuplicateError,
        e2_resolve::ResolveError,
        e3_type::{InferLiteralError, TypeError},
    },
    hir_def::{
        expressions::{
            expression::{Elementary, ExprKind, InitExpr, InitExprKind, PrimaryExpr},
            spec::SpecKind,
        },
        pous::variable::VariableDecl,
    },
    hir_ty::{head::init_inference::InitInference, infer::Infer, ty::Type},
};

impl<'db> InitInference<'db> {
    pub(crate) fn check_variables(&mut self, db: &'db dyn WorkspaceDataBase) {
        let variables = match self.scope.variables(db) {
            Some(vars) => vars,
            None => return,
        };

        // RETAIN/NON_RETAIN require instance storage: meaningless on a
        // stateless POU (FUNCTION/METHOD), in ANY of its sections (E0235).
        let scope_kind = get_scope(db, self.scope).kind;
        let stateless_pou = match scope_kind {
            ScopeKind::Pou(Pou::Function(_)) => Some("FUNCTION"),
            ScopeKind::MethodDecl(_) => Some("METHOD"),
            ScopeKind::MethodProt(_) => Some("METHOD prototype"),
            _ => None,
        };

        // `{extern}` legality. The pragma position is shared by every
        // pragma-carrying POU, so the FUNCTION-only rule is enforced here;
        // and an extern FUNCTION's interface is copies in, scalar results
        // out, with the import standing in for the body — so VAR_IN_OUT,
        // aggregate outputs and statements are each refused where they are
        // declared. (E0243/E0244.)
        let extern_fn = match scope_kind {
            ScopeKind::Pou(Pou::Function(f)) => f.extern_pragma(db).map(|(span, _)| (f, span)),
            ScopeKind::Pou(Pou::FunctionBlock(fb)) => {
                self.refuse_extern_on(db, fb.extern_pragma(db), "FUNCTION_BLOCK");
                self.refuse_test_on(db, fb.test_pragma(db), "FUNCTION_BLOCK");
                None
            }
            // CLASS/INTERFACE take no pragmas in the grammar — `{extern}`
            // there is a parse error before it can reach this check.
            ScopeKind::Program(p) => {
                self.refuse_extern_on(db, p.extern_pragma(db), "PROGRAM");
                self.refuse_test_on(db, p.test_pragma(db), "PROGRAM");
                None
            }
            ScopeKind::MethodDecl(m) => {
                self.refuse_extern_on(db, m.extern_pragma(db), "METHOD");
                self.refuse_test_on(db, m.test_pragma(db), "METHOD");
                None
            }
            _ => None,
        };
        if let Some((f, _span)) = extern_fn
            && let Some(first) = f.statements(db).first()
        {
            self.errors.push(
                ResolveError::ExternWithBody {
                    site: first.as_call_site(db),
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }
        // The return type is the import's LAST result, so it answers to the
        // same scalar rule as VAR_OUTPUT. Checking only the sections left the
        // one aggregate an extern can still name — a STRING or STRUCT return —
        // to be caught by an assertion in codegen instead.
        if let Some((f, _span)) = extern_fn
            && let Some(ret) = f.return_type(db)
            && !extern_scalar(db, ret.infer(db))
        {
            self.errors.push(
                ResolveError::ExternNonScalarReturn { func: f, ret: *ret }
                    .to_diagnostic(db, self.scope.file(db)),
            );
        }

        // Inside a FUNCTION or a value-returning METHOD the callable's own
        // name is the return value, so a variable declared with it (in any
        // case) is a second declaration: the body binds to the local, at the
        // local's type, while the signature still promises the return type's
        // — which is invalid wasm at exit 0, not a subtle bug.
        let return_value_name = match scope_kind {
            ScopeKind::Pou(Pou::Function(f)) => f
                .return_type(db)
                .is_some()
                .then(|| (f.get_name_ident(db).caseless(db), "FUNCTION")),
            ScopeKind::MethodDecl(m) => m
                .return_type(db)
                .is_some()
                .then(|| (m.get_name_ident(db).caseless(db), "METHOD")),
            _ => None,
        };

        let mut seen = FxHashMap::default();
        let mut first_variadic: Option<VariableDecl<'db>> = None;

        for var in variables {
            if let Some(pou_kind) = stateless_pou
                && var
                    .qualifier(db)
                    .intersects(crate::Qualifier::RETAIN | crate::Qualifier::NON_RETAIN)
            {
                self.errors.push(
                    ResolveError::RetainInStatelessPou {
                        var: *var,
                        pou_kind,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
            // A VAR_EXTERNAL aliases its VAR_GLOBAL's storage by NAME, so
            // the two declarations must agree about the TYPE, any type
            // (E0246). Cycle-safe here where a named global type resolves
            // freely; inside signature inference the same resolution
            // re-enters `infer_signature`. Absence is E0220, the signature's.
            if var.kind(db) == crate::hir_def::pous::variable::VariableKind::External
                && let Some(global) =
                    crate::hir_ty::index_graphs::external_var_lookup(db, var.get_name_ident(db))
            {
                let ext_ty = Type::resolve_spec(db, var.spec(db));
                let glob_ty = Type::resolve_spec(db, global.spec(db));
                if !ext_ty.is_never()
                    && !glob_ty.is_never()
                    && !same_storage_type(db, ext_ty, glob_ty)
                {
                    self.errors.push(
                        ResolveError::ExternalVarTypeMismatch {
                            var: *var,
                            external: ext_ty,
                            global: glob_ty,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
            // A declared location gets the SAME answer as a direct access:
            // the hardware is not implemented, so binding a variable to an
            // address cannot be honoured. Silently dropping the `AT` clause
            // handed the user an ordinary variable that never sees its input.
            //
            // TODO: lift this when an I/O band lands — `var.location` already
            // carries what a mapping would bind.
            if let Some(dv) = var.location(db) {
                self.errors.push(
                    ResolveError::DirectVariableUnsupported {
                        site: var.as_call_site(db),
                        address: compact_str::CompactString::from(dv.to_address(db)),
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
            if extern_fn.is_some() {
                use crate::hir_def::pous::variable::VariableKind;
                let forbidden = match var.kind(db) {
                    VariableKind::InOut => Some(ExternForbiddenKind::InOut),
                    VariableKind::Output if !extern_scalar(db, var.spec(db).infer(db)) => {
                        Some(ExternForbiddenKind::AggregateOutput)
                    }
                    _ => None,
                };
                if let Some(kind) = forbidden {
                    self.errors.push(
                        ResolveError::ExternForbiddenSection { var: *var, kind }
                            .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
            // Folded: `Count` and `count` are one identifier, so declaring
            // both is declaring the same variable twice.
            if let Some((ret_name, pou_kind)) = return_value_name
                && var.get_name_ident(db).caseless(db) == ret_name
            {
                self.errors.push(
                    DuplicateError::VariableIsReturnValue {
                        var: *var,
                        pou_kind,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
            }
            match seen.get(&var.get_name_ident(db).caseless(db)) {
                Some(prev) => {
                    self.errors.push(
                        DuplicateError::Variable {
                            var1: *var,
                            var2: *prev,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
                None => {
                    seen.insert(var.get_name_ident(db).caseless(db), *var);
                }
            }

            self.check_spec(db, var.spec(db));

            let var_type = var.spec(db).infer(db);

            if var.variadic(db) {
                if !var_type.normalize(db).can_be_variadic(db) {
                    self.errors.push(
                        ResolveError::NonVariadicTypeForVariable {
                            var: *var,
                            typ: var_type,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }

                if !var.is_input(db) {
                    self.errors.push(
                        ResolveError::VariadicNotInInput { var: *var }
                            .to_diagnostic(db, self.scope.file(db)),
                    );
                }

                if let Some(first) = first_variadic {
                    self.errors.push(
                        ResolveError::MultipleVariadicVariables {
                            first,
                            second: *var,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                } else {
                    first_variadic = Some(*var);
                }
            }

            if let Some(init_expr) = var.init(db) {
                self.init_expr_result.resolve_init_expr(
                    db,
                    init_expr,
                    &mut self.body_infer_result,
                    var_type,
                );

                // Check string literal length for sized string specs
                self.check_sized_string_init(db, var.spec(db).kind(db), init_expr);
            }
        }

        // If there's a variadic parameter, it must be the only VAR_INPUT parameter
        if let Some(variadic_var) = first_variadic {
            for var in variables {
                if var.is_input(db) && !var.variadic(db) {
                    self.errors.push(
                        ResolveError::VariadicMixedWithOtherInputs {
                            variadic_var,
                            other_var: *var,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
        }
    }

    fn check_sized_string_init(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        spec_kind: &SpecKind<'db>,
        init: InitExpr<'db>,
    ) {
        let max_len_expr = match spec_kind {
            SpecKind::SizedString(length) => length,
            _ => return,
        };

        let max_len = match max_len_expr.as_range(db) {
            Some(len) => len,
            None => return,
        };

        // Only check simple constant expression initializers
        let InitExprKind::ConstantExpr(expr) = init.kind(db) else {
            return;
        };

        let ExprKind::PrimaryExpr(PrimaryExpr::Literal(elem)) = expr.expr(db) else {
            return;
        };

        // Both single-quoted and (legacy) double-quoted forms now resolve
        // to STRING; single-byte payload measurement covers both.
        let actual_len = match elem {
            Elementary::String(s) => s.as_single_string(db).ok().map(|v| v.len()),
            _ => None,
        };

        if let Some(actual_len) = actual_len
            && actual_len as u64 > max_len
        {
            let err = InferLiteralError::Invalid_STRING_Length {
                max: max_len,
                got: actual_len,
            };
            let target =
                Type::Elementary(crate::hir_def::expressions::spec::ElementarySpec::String);
            self.errors.push(
                TypeError::InferLiteralError {
                    expr,
                    source: None,
                    target,
                    err,
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }
    }
}

impl<'db> InitInference<'db> {
    /// Push E0244 when `pragma` is present: `{extern}` on a POU kind that
    /// cannot be an import.
    /// `{test}` is FUNCTION-only, like `{extern}` (E0252): the runner calls
    /// a `()` entry, which no other POU kind has.
    fn refuse_test_on(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        pragma: Option<&'db crate::hir_def::interned::identifier::SpanIdent<'db>>,
        pou_kind: &'static str,
    ) {
        if let Some(anchor) = pragma {
            self.errors.push(
                ResolveError::TestOutsideFunction {
                    anchor: *anchor,
                    pou_kind,
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }
    }

    fn refuse_extern_on(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        pragma: Option<(
            &'db crate::hir_def::interned::identifier::SpanIdent<'db>,
            &'db crate::hir_def::pous::pragma::ExternPragma,
        )>,
        pou_kind: &'static str,
    ) {
        if let Some((anchor, _)) = pragma {
            self.errors.push(
                ResolveError::ExternOutsideFunction {
                    anchor: *anchor,
                    pou_kind,
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }
    }
}

/// Can this type ride a WASM result? Scalars only: every elementary except
/// STRING (memory-resident), plus enums and subranges, which store at a
/// scalar lane. Aggregates have no result type to ride.
fn extern_scalar<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> bool {
    use crate::hir_def::expressions::spec::ElementarySpec;
    match ty.normalize(db) {
        Type::Elementary(es) => es != ElementarySpec::String,
        Type::Enum(_) | Type::SubRange(_) => true,
        _ => false,
    }
}

/// Whether two declarations name the SAME storage type, structurally where
/// the declarations are spelled inline: two `ARRAY[0..2] OF INT` specs are
/// distinct nodes but the same type. Subranges compare by base and bounds;
/// named types (structs, enums, FBs) by the declaration they resolve to.
fn same_storage_type<'db>(db: &'db dyn WorkspaceDataBase, a: Type<'db>, b: Type<'db>) -> bool {
    let bounds = |t: Type<'db>| {
        t.as_subrange(db)
            .map(|s| crate::hir_ty::infer::const_eval::subrange_bounds(db, s))
    };
    if bounds(a) != bounds(b) {
        return false;
    }
    match (a.normalize(db), b.normalize(db)) {
        (Type::Array(x), Type::Array(y)) => {
            crate::hir_ty::infer::const_eval::array_dimensions(db, x)
                == crate::hir_ty::infer::const_eval::array_dimensions(db, y)
                && same_storage_type(
                    db,
                    Type::resolve_spec(db, x.of_type(db)),
                    Type::resolve_spec(db, y.of_type(db)),
                )
        }
        (x, y) => x == y,
    }
}
