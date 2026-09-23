use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::HasPragmas;
use crate::check::errors::e08_call::CallError;
use crate::check::errors::e14_config::ConfigError;
use crate::check::errors::e15_pragma::ExportForbiddenKind;
use crate::check::errors::e15_pragma::ExternForbiddenKind;
use crate::check::errors::e15_pragma::PragmaError;
use crate::hir_def::{pous::pou::Pou, scope::ScopeKind, semantic_index::get_scope};
use crate::{
    HasName, HirNodeInfo,
    check::errors::{
        ToIdeDiagnostic,
        e01_duplicates::DuplicateError,
        e02_resolve::ResolveError,
        e03_type::{InferLiteralError, TypeError},
    },
    hir_def::{
        expressions::{
            expression::{Elementary, ExprKind, InitExpr, InitExprKind, PrimaryExpr},
            spec::Spec,
        },
        pous::variable::VariableDecl,
    },
    hir_ty::{head::init_inference::InitInference, infer::Infer, ty::Type},
};

impl<'db> InitInference<'db> {
    /// A variable declared `AT %I*`, `%Q*` or `%M*` in a PROGRAM, a
    /// FUNCTION_BLOCK or a CLASS: VAR_CONFIG gives each instance its address
    /// (E1424, E1425), so only what the declaration alone decides is checked
    /// here, RETAIN (E1420); the grammar gives it no initial value. Returns
    /// whether `var` was one; anywhere else a partial address has nothing to
    /// complete it, and it stays E1417.
    fn check_partly_located(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        var: &VariableDecl<'db>,
        dv: crate::hir_def::pous::variable::DirectVariable<'db>,
    ) -> bool {
        let in_instance = matches!(
            get_scope(db, var.get_scope_id(db)).kind,
            ScopeKind::Program(_) | ScopeKind::Pou(Pou::FunctionBlock(_) | Pou::Class(_))
        );
        if !dv.is_area_only(db) || !var.is_var(db) || !in_instance {
            return false;
        }
        if var.qualifier(db).contains(crate::Qualifier::RETAIN) {
            self.errors.push(
                ConfigError::RetainOnIoLocation {
                    var: *var,
                    address: compact_str::CompactString::from(dv.to_address(db)),
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }
        true
    }

    /// E1425: an instance whose type holds a variable declared `AT %I*`,
    /// held where no VAR_CONFIG path can name it, or where a call copies over
    /// it: an array's element, a STRUCT's field, a VAR_GLOBAL, an instance
    /// made for each call, a VAR_INPUT. Returns whether it reported.
    fn check_unreachable_partly(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        var: &VariableDecl<'db>,
    ) -> bool {
        use crate::check::errors::e14_config::{PartlyUnlocated, UnreachablePlace};
        if var.is_external(db) || var.is_in_out(db) {
            return false;
        }
        let mut ty = var.spec(db).infer(db).normalize(db);
        let mut in_array = false;
        while let Type::Array(array) = ty {
            in_array = true;
            ty = array.of_type(db).infer(db).normalize(db);
        }
        let (names, member, in_struct) = match crate::hir_ty::head::inheritance::pou_of_type(db, ty)
        {
            Some(pou) => {
                let paths = crate::hir_ty::head::inheritance::partly_located_members(db, pou);
                let Some(path) = paths.first() else {
                    return false;
                };
                let Some(member) = path.last() else {
                    return false;
                };
                (
                    path.iter().map(|m| m.name(db)).collect::<Vec<_>>(),
                    *member,
                    false,
                )
            }
            None => match crate::hir_ty::head::inheritance::partly_located_in_struct(
                db,
                ty,
                &mut Vec::new(),
            ) {
                Some((names, member)) => (names, member, true),
                None => return false,
            },
        };
        let per_call = var.is_temp(db)
            || matches!(
                get_scope(db, var.get_scope_id(db)).kind,
                ScopeKind::Pou(Pou::Function(_)) | ScopeKind::MethodDecl(_)
            );
        let place = if in_struct {
            UnreachablePlace::Struct
        } else if in_array {
            UnreachablePlace::Array
        } else if var.is_global(db) {
            UnreachablePlace::Global
        } else if per_call {
            UnreachablePlace::PerCall
        } else if var.is_input(db) {
            UnreachablePlace::Input
        } else {
            return false;
        };
        let member_path = names
            .iter()
            .map(|n| n.text(db).to_string())
            .collect::<Vec<_>>()
            .join(".");
        let address = member
            .location(db)
            .map(|dv| dv.to_address(db))
            .unwrap_or_default();
        self.errors.push(
            ConfigError::PartlyLocatedUnlocated(PartlyUnlocated::Unreachable {
                var: *var,
                member: compact_str::CompactString::from(member_path),
                address: compact_str::CompactString::from(address),
                place,
            })
            .to_diagnostic(db, self.scope.file(db)),
        );
        true
    }

    /// E1420: a RETAIN instance holding a variable declared `AT %M*`, which
    /// points at its marker and would silently not be retained with the rest.
    /// An `%I*` or `%Q*` member is not refused: neither area persists anyway.
    fn check_retain_holds_marker(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        var: &VariableDecl<'db>,
    ) {
        if !var.qualifier(db).contains(crate::Qualifier::RETAIN) {
            return;
        }
        let Some(pou) =
            crate::hir_ty::head::inheritance::pou_of_type(db, var.spec(db).infer(db).normalize(db))
        else {
            return;
        };
        let marker = crate::hir_ty::head::inheritance::partly_located_members(db, pou)
            .iter()
            .find(|path| {
                path.last()
                    .and_then(|m| m.location(db))
                    .and_then(|dv| dv.area(db))
                    == Some(crate::hir_def::pous::variable::LocationArea::Marker)
            });
        if let Some(path) = marker {
            let member = path
                .iter()
                .map(|m| m.name(db).text(db).to_string())
                .collect::<Vec<_>>()
                .join(".");
            self.errors.push(
                ConfigError::RetainHoldsPartlyLocated {
                    var: *var,
                    member: compact_str::CompactString::from(member),
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }
    }

    pub(crate) fn check_variables(&mut self, db: &'db dyn WorkspaceDataBase) {
        let variables = match self.scope.variables(db) {
            Some(vars) => vars,
            None => return,
        };

        // RETAIN/NON_RETAIN require instance storage: meaningless on a
        // stateless POU (FUNCTION/METHOD), in ANY of its sections (E0208).
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
        // declared. (E1502/E1501.)
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
        // `{export}` legality, enforced where `{extern}`'s is. (E1508/E1509.)
        match scope_kind {
            ScopeKind::Pou(Pou::Function(f)) => self.check_export(db, f),
            ScopeKind::Pou(Pou::FunctionBlock(fb)) => {
                self.refuse_export_on(db, fb.export_pragma(db), "FUNCTION_BLOCK");
            }
            ScopeKind::Program(p) => self.refuse_export_on(db, p.export_pragma(db), "PROGRAM"),
            ScopeKind::MethodDecl(m) => self.refuse_export_on(db, m.export_pragma(db), "METHOD"),
            _ => {}
        }
        if let Some((f, _span)) = extern_fn
            && let Some(first) = f.statements(db).first()
        {
            self.errors.push(
                PragmaError::ExternWithBody {
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
                PragmaError::ExternNonScalarReturn { func: f, ret: *ret }
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
            if !self.check_unreachable_partly(db, var) {
                self.check_retain_holds_marker(db, var);
            }
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
            // (E0207). Cycle-safe here where a named global type resolves
            // freely; inside signature inference the same resolution
            // re-enters `infer_signature`. Absence is E0206, the signature's.
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
            // A VAR_GLOBAL or a PROGRAM's VAR with an `AT` clause is storage
            // in one of the three I/O bands. Any other POU's variables are
            // fields of each of its instances, so a single address cannot be
            // theirs. Same answer for an address with no area letter (`%Z0`),
            // no width letter (`%I0`) or none at all (`%I*`, which needs the
            // binding VAR_CONFIG supplies): nothing maps them (E1417).
            if let Some(dv) = var.location(db)
                && !self.check_partly_located(db, var, dv)
            {
                let address = compact_str::CompactString::from(dv.to_address(db));
                // A library names no address: it is code for any machine.
                let in_library =
                    crate::check::check_duplicates::is_library_file(db, self.scope.file(db));
                let banded = dv.area(db).filter(|_| {
                    !in_library
                        && ((var.kind(db) == crate::hir_def::pous::variable::VariableKind::Global
                            && crate::hir_ty::infer::normalize::names_a_band(db, dv))
                            || var.is_program_located(db))
                });
                match banded {
                    // `%I` is copied in before every scan and `%Q` read back
                    // after it, so neither can ALSO be restored from the
                    // retain band at startup. `%M` is the area that may.
                    Some(area)
                        if area != crate::hir_def::pous::variable::LocationArea::Marker
                            && var.qualifier(db).contains(crate::Qualifier::RETAIN) =>
                    {
                        self.errors.push(
                            ConfigError::RetainOnIoLocation { var: *var, address }
                                .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                    Some(_) => {
                        // An address is one channel. Two declarations bound
                        // to it each get storage of their own, so a write
                        // through one is invisible through the other and a
                        // host binding by address finds it twice. They may be
                        // a VAR_GLOBAL and a PROGRAM's VAR, in two files, which
                        // have no order: each is reported, like E1402, and
                        // points at the smallest of the others.
                        let other = crate::hir_def::pous::variable::LocatedAddress::of(db, dv)
                            .map(|a| crate::hir_ty::index_graphs::located_declarations_at(db, &a))
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|other| other != var)
                            .min_by_key(|other| {
                                (
                                    other.get_scope_id(db).file(db).url(db).to_string(),
                                    other.get_name_span(db).start_byte,
                                )
                            });
                        if let Some(other) = other {
                            self.errors.push(
                                ConfigError::DuplicateLocation {
                                    var: *var,
                                    other,
                                    address: address.clone(),
                                }
                                .to_diagnostic(db, self.scope.file(db)),
                            );
                        }
                        // `__init` would write it, and the host's copy-in
                        // before the first scan overwrites it unread.
                        if var.init(db).is_some()
                            && dv.area(db)
                                == Some(crate::hir_def::pous::variable::LocationArea::Input)
                        {
                            self.errors.push(
                                ConfigError::WriteToInputLocation {
                                    site: var.as_call_site(db),
                                    address: address.clone(),
                                    via: crate::check::errors::e14_config::InputWriteRoute::Initializer,
                                }
                                .to_diagnostic(db, self.scope.file(db)),
                            );
                        }
                        // One value as wide as the address: an elementary
                        // type of that width, reached through any alias. An
                        // enum, a subrange, an aggregate or a STRING has no
                        // such width and is refused rather than guessed at.
                        let declared = Type::resolve_spec(db, var.spec(db));
                        let declared_bits = if declared.as_subrange(db).is_some() {
                            None
                        } else {
                            located_width(declared.normalize(db))
                        };
                        if !declared.is_never()
                            && let Some((_, address_bits)) = dv
                                .size_letter(db)
                                .and_then(crate::hir_ty::infer::normalize::access_size)
                            && declared_bits != Some(address_bits)
                        {
                            self.errors.push(
                                ConfigError::LocationWidthMismatch {
                                    var: *var,
                                    address,
                                    address_bits,
                                    declared_bits,
                                    declared,
                                }
                                .to_diagnostic(db, self.scope.file(db)),
                            );
                        }
                        // Located inside a wider address the workspace
                        // mentions, the variable is that address's bits and
                        // has no storage of its own (E1423), so persistence
                        // and a startup value are the owner's to declare.
                        if let Some(located) =
                            crate::hir_def::pous::variable::LocatedAddress::of(db, dv)
                            && let Some(view) =
                                crate::hir_ty::index_graphs::located_view(db, &located)
                        {
                            use crate::check::errors::e14_config::{
                                OwnerDeclaration, WiderAddressUse,
                            };
                            // Where the RETAIN or the value can go instead.
                            let owner = if crate::hir_ty::index_graphs::located_declaration(
                                db,
                                &view.owner,
                            )
                            .is_some()
                            {
                                OwnerDeclaration::Declared
                            } else if crate::hir_ty::index_graphs::config_located(db)
                                .any(|a| *a == view.owner)
                            {
                                OwnerDeclaration::Configured
                            } else {
                                OwnerDeclaration::Bare
                            };
                            let mut refuse = |usage| {
                                self.errors.push(
                                    ConfigError::PartOfWiderAddress {
                                        site: var.as_call_site(db),
                                        address: located.text.clone(),
                                        owner: view.owner.text.clone(),
                                        usage,
                                    }
                                    .to_diagnostic(db, self.scope.file(db)),
                                )
                            };
                            // Only `%M` gets here RETAIN: E1420 took `%I`/`%Q`.
                            if var.qualifier(db).contains(crate::Qualifier::RETAIN) {
                                refuse(WiderAddressUse::Retain(owner));
                            }
                            // On an input E1419 below says it.
                            if var.init(db).is_some()
                                && located.area
                                    != crate::hir_def::pous::variable::LocationArea::Input
                            {
                                refuse(WiderAddressUse::Initializer(owner));
                            }
                        }
                    }
                    None => {
                        use crate::check::errors::e14_config::UnlocatableAddress;
                        // A well-formed address in a POU is the POU's fault;
                        // anything else is the address's.
                        let why = if in_library && !dv.partly(db) {
                            UnlocatableAddress::InLibrary
                        } else if dv.partly(db) && !dv.is_area_only(db) {
                            UnlocatableAddress::NotAreaOnly
                        } else if dv.partly(db) {
                            UnlocatableAddress::Incomplete
                        } else if crate::hir_ty::infer::normalize::names_a_band(db, dv) {
                            UnlocatableAddress::InPou
                        } else {
                            UnlocatableAddress::Malformed
                        };
                        self.errors.push(
                            ConfigError::DirectVariableUnsupported {
                                site: var.as_call_site(db),
                                address,
                                why,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        )
                    }
                }
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
                        PragmaError::ExternForbiddenSection { var: *var, kind }
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
                        CallError::NonVariadicTypeForVariable {
                            var: *var,
                            typ: var_type,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }

                if let Some(first) = first_variadic {
                    self.errors.push(
                        CallError::MultipleVariadicVariables {
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
                self.check_string_init(db, var.spec(db), init_expr);
            }
        }

        // If there's a variadic parameter, it must be the only VAR_INPUT parameter
        if let Some(variadic_var) = first_variadic {
            for var in variables {
                if var.is_input(db) && !var.variadic(db) {
                    self.errors.push(
                        CallError::VariadicMixedWithOtherInputs {
                            variadic_var,
                            other_var: *var,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
        }
    }

    /// A STRING initializer that does not fit its capacity, which the
    /// assignment check already refuses in a body. Reading only a written
    /// `STRING[N]` let a literal past the DEFAULT capacity through, and the
    /// codegen then truncated it with nothing said.
    fn check_string_init(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        spec: Spec<'db>,
        init: InitExpr<'db>,
    ) {
        if !matches!(
            spec.infer(db).normalize(db),
            Type::Elementary(crate::hir_def::expressions::spec::ElementarySpec::String)
        ) {
            return;
        }

        let max_len = u64::from(
            crate::hir_ty::infer::normalize::declared_string_capacity(db, spec)
                .unwrap_or(crate::hir_ty::infer::normalize::DEFAULT_STRING_CAPACITY),
        );

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
            Elementary::String(s) | Elementary::InferString(s) => {
                s.as_single_string(db).ok().map(|v| v.len())
            }
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
    /// Push E1501 when `pragma` is present: `{extern}` on a POU kind that
    /// cannot be an import.
    /// `{test}` is FUNCTION-only, like `{extern}` (E1503): the runner calls
    /// a `()` entry, which no other POU kind has.
    fn refuse_test_on(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        pragma: Option<&'db crate::hir_def::interned::identifier::SpanIdent<'db>>,
        pou_kind: &'static str,
    ) {
        if let Some(anchor) = pragma {
            self.errors.push(
                PragmaError::TestOutsideFunction {
                    anchor: *anchor,
                    pou_kind,
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }
    }

    /// Push E1508 when `pragma` is present: `{export}` on a POU kind the
    /// host could not call. A PROGRAM is exported for the schedule already;
    /// the others need an instance.
    fn refuse_export_on(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        pragma: Option<&'db crate::hir_def::interned::identifier::SpanIdent<'db>>,
        pou_kind: &'static str,
    ) {
        if let Some(anchor) = pragma {
            self.errors.push(
                PragmaError::ExportOutsideFunction {
                    anchor: *anchor,
                    pou_kind,
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }
    }

    /// Push E1509 when an `{export}` FUNCTION has no single export to give:
    /// it is an import, a test, or one declaration the lowering turns into
    /// several functions. The first reason that applies is the one reported.
    fn check_export(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        func: crate::hir_def::pous::function::Function<'db>,
    ) {
        let Some(anchor) = func.export_pragma(db) else {
            return;
        };
        let kind = if func.extern_pragma(db).is_some() {
            ExportForbiddenKind::Extern
        } else if func.is_test(db) {
            ExportForbiddenKind::Test
        } else if func.variables(db).iter().any(|v| {
            matches!(
                v.kind(db),
                crate::hir_def::pous::variable::VariableKind::Input
                    | crate::hir_def::pous::variable::VariableKind::InOut
            ) && matches!(v.spec(db).infer(db).normalize(db), Type::Interface(_))
        }) {
            ExportForbiddenKind::InterfaceParam
        } else if func.variables(db).iter().any(|v| v.variadic(db)) {
            ExportForbiddenKind::Variadic
        } else if crate::hir_ty::resolver::name::overload_discriminant(db, func).is_some() {
            ExportForbiddenKind::Overloaded
        } else {
            return;
        };
        self.errors.push(
            PragmaError::ExportForbidden {
                anchor: *anchor,
                func,
                kind,
            }
            .to_diagnostic(db, self.scope.file(db)),
        );
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
                PragmaError::ExternOutsideFunction {
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
pub(crate) fn same_storage_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    a: Type<'db>,
    b: Type<'db>,
) -> bool {
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

/// The width a located variable of type `ty` fills: every elementary type
/// but STRING has one. `Type::get_size` leaves CHAR out, and it is 8 bits.
pub(crate) fn located_width(ty: Type<'_>) -> Option<usize> {
    match ty {
        Type::Elementary(crate::hir_def::expressions::spec::ElementarySpec::Char) => Some(8),
        Type::Elementary(_) => match ty.get_size() {
            crate::hir_ty::ty::Size::Size(bits) => Some(bits),
            crate::hir_ty::ty::Size::Null => None,
        },
        _ => None,
    }
}

