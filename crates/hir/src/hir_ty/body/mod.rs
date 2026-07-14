use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    CallSite, HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, ExprKind, InitExprKind, ParamAssign, PathExpr, PrimaryExpr,
                RefValue, VariableAccess, VariableAccessKind,
            },
            invocation::Invocation,
            spec::SpecKind,
            statement::Stmt,
        },
        interned::identifier::Ident,
        pous::{
            generics::derive_generic_params,
            pou::Pou,
            variable::{DirectVariable, VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
        using::Using,
    },
    hir_ty::{
        body::statements::{NestedScope, StmtsResolverCtx, check_preprocess},
        infer::Infer,
        resolver::{
            Resolver,
            name::{NameResolution, resolve_name},
        },
        ty::Type,
    },
};

pub mod statements;

#[tracing::instrument(skip(db))]
#[salsa::tracked(returns(ref))]
pub fn infer_body<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
) -> BodyInferenceResult<'db> {
    let mut result = BodyInferenceResult::new(scope);
    let ctx = StmtsResolverCtx::new(scope);

    // Only Scopes with bodies can have statements
    let statements = match get_scope(db, scope).kind {
        ScopeKind::Pou(pou) => match pou {
            Pou::Function(f) => f.statements(db),
            Pou::FunctionBlock(fb) => fb.statements(db),
            _ => return result,
        },
        ScopeKind::MethodDecl(m) => m.stmts(db),
        ScopeKind::Program(program) => program.statements(db),
        _ => return result,
    };

    // Initialize null state tracking for REF_TO local variables
    init_ref_null_states(db, scope, &mut result);

    // Record method locals/params that shadow an owner FB/Class member.
    init_method_member_shadows(db, scope, &mut result);

    // Populate fb_any_resolutions from explicit `<T>` type arguments on VARs.
    // This replaces the old call-site inference path (removed from
    // `resolver::func_call`) so monomorphization works even without a call
    // - e.g. an FB instance stored in a struct field.
    init_fb_generic_bindings(db, scope, &mut result);

    let resolver = Resolver::for_scope(db, scope);

    ctx.check_statements(db, resolver, statements, NestedScope::None, &mut result);

    check_preprocess(
        db,
        scope,
        statements,
        &FxHashMap::default(),
        false,
        &mut result,
    );

    result
}

/// A method local/parameter that has the same name as a member of the owner
/// FB/Class shadows it (the local wins, per HIR name resolution). Record each
/// (method variable → shadowed member) for the linter.
fn init_method_member_shadows<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    result: &mut BodyInferenceResult<'db>,
) {
    let scope_data = get_scope(db, scope);
    let ScopeKind::MethodDecl(method) = scope_data.kind else {
        return;
    };
    let Some(parent) = scope_data.parent else {
        return;
    };
    let members: &[VariableDecl<'db>] = match get_scope(db, parent).kind {
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => fb.variables(db),
        ScopeKind::Pou(Pou::Class(cl)) => cl.variables(db),
        _ => return,
    };
    for mvar in method.variables(db) {
        let name = mvar.get_name_ident(db);
        if let Some(member) = members.iter().find(|m| m.get_name_ident(db) == name) {
            result.method_shadowed_members.insert(*mvar, *member);
        }
    }
}

/// Scan all variables in the scope and initialize null state tracking
/// for REF_TO variables (excluding VAR_INPUT and VAR_IN_OUT which are caller's responsibility).
fn init_ref_null_states<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    result: &mut BodyInferenceResult<'db>,
) {
    let variables = match get_scope(db, scope).kind {
        ScopeKind::Pou(pou) => match pou {
            Pou::Function(f) => f.variables(db),
            Pou::FunctionBlock(fb) => fb.variables(db),
            _ => return,
        },
        ScopeKind::MethodDecl(m) => m.variables(db),
        ScopeKind::Program(p) => p.variables(db),
        _ => return,
    };

    for var in variables {
        // Skip input/in_out - caller's responsibility
        if matches!(var.kind(db), VariableKind::Input | VariableKind::InOut) {
            continue;
        }

        // Check if this variable is REF_TO
        if matches!(var.spec(db).infer(db), Type::RefTo(_)) {
            let var_site = var.as_call_site(db);
            let state = match var.init(db) {
                Some(init) => {
                    if is_null_init(db, &init.kind(db)) {
                        NullState::Null(init.as_call_site(db))
                    } else {
                        NullState::NonNull
                    }
                }
                None => NullState::Uninitialized(var_site),
            };
            result.ref_null_state.insert(*var, state);
        }
    }
}

/// Populate `fb_any_resolutions` from each VAR whose spec is a `Target`
/// carrying explicit `<T>` type arguments - e.g. `VAR c : Counter<INT>;`.
///
/// For every `ANY_*` field on the referenced FB/Class, we find the parameter
/// slot whose bound matches that kind and record the arg as the concrete
/// binding for that field. Multiple fields of the same `ANY_*` kind share
/// the same slot
///
/// Errors like arity or bound mismatch are already reported at `check_spec`
/// time; this function bails out silently in those cases.
fn init_fb_generic_bindings<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    result: &mut BodyInferenceResult<'db>,
) {
    let variables = match get_scope(db, scope).kind {
        ScopeKind::Pou(pou) => match pou {
            Pou::Function(f) => f.variables(db),
            Pou::FunctionBlock(fb) => fb.variables(db),
            _ => return,
        },
        ScopeKind::MethodDecl(m) => m.variables(db),
        ScopeKind::Program(p) => p.variables(db),
        _ => return,
    };

    for var in variables {
        let SpecKind::Target(target) = var.spec(db).kind(db) else {
            continue;
        };
        if target.type_args.is_empty() {
            continue;
        }

        let pou = match resolve_name(db, &target.path, scope) {
            NameResolution::Pou(p, _) => p,
            _ => continue,
        };

        let params = derive_generic_params(db, &pou);
        if params.len() != target.type_args.len() {
            continue;
        }

        let pou_fields = match pou {
            Pou::FunctionBlock(fb) => fb.variables(db),
            Pou::Class(c) => c.variables(db),
            _ => continue,
        };

        for field in pou_fields {
            let SpecKind::Simple(field_any) = field.spec(db).kind(db) else {
                continue;
            };
            if !field_any.is_any() {
                continue;
            }
            let Some(idx) = params.iter().position(|p| p.bound == *field_any) else {
                continue;
            };
            if let Type::Elementary(concrete) = Type::resolve_spec(db, target.type_args[idx])
                && !concrete.is_any()
            {
                result
                    .fb_any_resolutions
                    .insert((*var, field.name(db)), concrete);
            }
        }
    }
}

/// Check if an initializer expression is NULL.
fn is_null_init<'db>(db: &'db dyn WorkspaceDataBase, init: &InitExprKind<'db>) -> bool {
    match init {
        InitExprKind::ConstantExpr(expr) => {
            matches!(
                expr.expr(db),
                ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                    value: RefValue::Null
                })
            )
        }
        _ => false,
    }
}

/// Nullability state for REF_TO variables, tracked during linear statement flow.
///
/// The [`CallSite`] carried by `Uninitialized` and `Null` points to
/// the source location that caused the state (declaration site or NULL assignment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum NullState<'db> {
    /// Variable was never assigned (default for REF_TO without initializer).
    /// CallSite points to the variable declaration.
    Uninitialized(CallSite<'db>),
    /// Variable was explicitly assigned NULL.
    /// CallSite points to the NULL assignment or NULL initializer.
    Null(CallSite<'db>),
    /// Variable was assigned a non-null value.
    NonNull,
}

impl<'db> NullState<'db> {
    /// Join two null states at a control flow merge point.
    /// If both branches agree, use that state. Otherwise, use the more conservative (nullable) one.
    pub fn join(self, other: NullState<'db>) -> NullState<'db> {
        match (self, other) {
            // Both agree → keep
            (NullState::NonNull, NullState::NonNull) => NullState::NonNull,
            // Any nullable state wins
            (s @ NullState::Null(_), _) | (_, s @ NullState::Null(_)) => s,
            (s @ NullState::Uninitialized(_), _) | (_, s @ NullState::Uninitialized(_)) => s,
        }
    }
}

/// Result of body inference
///
/// When the this struct is emitted via the [`infer_body`] query, it is important to note 2 things about the type mappings:
///
/// 1. The types mapped to expressions and invocations are the types *before* any normalization or adjustments are applied.
/// see the note on normalization in the normalize module.
///
/// 2. There should be no [`Type::Infer`] types in the mappings. All types should be fully resolved,
/// those that can't be resolved will be represented as [`Type::Never`].
#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct BodyInferenceResult<'db> {
    // Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,

    // Mapping from parameter assignments to variables
    pub variable_of_param: FxHashMap<ParamAssign<'db>, VariableDecl<'db>>,

    // For variadic parameters, stores the 1-based position index
    pub variadic_position: FxHashMap<ParamAssign<'db>, usize>,

    // Mapping of direct variables to their types
    pub type_of_direct_variable: FxHashMap<DirectVariable<'db>, Type<'db>>,

    // Mapping from invocations to their resolved types.
    pub type_of_invocation: FxHashMap<Invocation<'db>, Type<'db>>,

    // Mapping from path expressions to their resolved types.
    pub type_of_path_expr: FxHashMap<PathExpr<'db>, Type<'db>>,

    // Mapping from expressions to their resolved types.
    pub type_of_expr: FxHashMap<Expr<'db>, Type<'db>>,

    // Mapping from path expressions to their adjustment sequences.
    pub path_expr_adjustments: FxHashMap<PathExpr<'db>, Vec<Adjustment<'db>>>,

    /// Resolved ANY_* type substitutions for FB instances.
    /// Key: (FB variable declaration, ANY_* variable name in the FB)
    /// Value: concrete ElementarySpec resolved from call-site arguments.
    ///
    /// Example: `VAR timer: CTU; END_VAR; timer(PV := 5);`
    /// → `(timer_var_decl, "PV") → ElementarySpec::Int`
    pub fb_any_resolutions:
        FxHashMap<(VariableDecl<'db>, Ident), crate::hir_def::expressions::spec::ElementarySpec>,

    // Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,

    // Set of variables that were referenced in the body.
    // Populated during statement resolution for use by the linter.
    pub variables_used: FxHashSet<VariableDecl<'db>>,

    // Set of USING directives that were actually used to resolve a name.
    // Populated during name resolution for use by the linter.
    pub usings_used: FxHashSet<Using<'db>>,

    // Variables that shadow a POU with the same name.
    // Populated during statement resolution for use by the linter.
    pub variables_shadowing: FxHashMap<VariableDecl<'db>, Pou<'db>>,

    // Method locals/params that shadow a member of the owner FB/Class (same
    // name). Maps the method variable → the shadowed member. Populated for
    // method bodies for use by the linter.
    pub method_shadowed_members: FxHashMap<VariableDecl<'db>, VariableDecl<'db>>,

    // Config/resource VAR_GLOBALs accessed directly by name without a matching
    // VAR_EXTERNAL declaration (direct access). Resolution still
    // succeeds; the linter warns, since strict IEC wants an explicit
    // VAR_EXTERNAL. Records (access path expr, resolved global decl).
    pub globals_without_external: Vec<(PathExpr<'db>, VariableDecl<'db>)>,

    // Function calls whose return value is discarded.
    // Populated during statement resolution for use by the linter.
    pub unused_return_types: Vec<(Stmt<'db>, Type<'db>)>,

    // Statements that are just expressions with no side effects.
    // Populated during statement resolution for use by the linter.
    pub effectless_statements: Vec<Stmt<'db>>,

    // CASE statements without an ELSE clause.
    // Populated during statement resolution for use by the linter.
    pub case_without_else: Vec<Stmt<'db>>,

    // Statements that are unreachable (after RETURN/EXIT/CONTINUE).
    // Populated during statement resolution for use by the linter.
    pub dead_code_statements: Vec<Stmt<'db>>,

    // FOR loops where step sign mismatches bounds direction.
    // Populated during statement resolution for use by the linter.
    pub mismatched_for_step: Vec<Stmt<'db>>,

    // Null state tracking for REF_TO variables.
    // Tracks whether a reference variable is initialized / null / non-null
    // through linear statement flow.
    pub ref_null_state: FxHashMap<VariableDecl<'db>, NullState<'db>>,
}

impl<'db> BodyInferenceResult<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        Self {
            scope,
            variable_of_param: FxHashMap::default(),
            variadic_position: FxHashMap::default(),
            type_of_direct_variable: FxHashMap::default(),
            type_of_invocation: FxHashMap::default(),
            type_of_expr: FxHashMap::default(),
            type_of_path_expr: FxHashMap::default(),
            path_expr_adjustments: FxHashMap::default(),
            errors: Vec::new(),
            variables_used: FxHashSet::default(),
            usings_used: FxHashSet::default(),
            variables_shadowing: FxHashMap::default(),
            method_shadowed_members: FxHashMap::default(),
            globals_without_external: Vec::new(),
            unused_return_types: Vec::new(),
            effectless_statements: Vec::new(),
            case_without_else: Vec::new(),
            dead_code_statements: Vec::new(),
            mismatched_for_step: Vec::new(),
            ref_null_state: FxHashMap::default(),
            fb_any_resolutions: FxHashMap::default(),
        }
    }

    pub(super) fn get_type_of_expr(&self, expr: Expr<'db>) -> Type<'db> {
        self.type_of_expr.get(&expr).copied().unwrap_or_default()
    }

    pub fn type_of_expr_with_adjustments(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: Expr<'db>,
    ) -> Type<'db> {
        match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(var)) => {
                self.type_of_variable_access_with_adjustments(db, *var)
            }
            _ => self.type_of_expr.get(&expr).copied().unwrap_or_default(),
        }
    }

    pub fn type_of_variable_access_with_adjustments(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var_access: VariableAccess<'db>,
    ) -> Type<'db> {
        match var_access.kind(db) {
            VariableAccessKind::Symbolic(sym) => self.type_of_begin_expr_with_adjustments(db, sym),
            VariableAccessKind::Direct(dv) => self
                .type_of_direct_variable
                .get(&dv)
                .copied()
                .unwrap_or_default(),
        }
    }

    pub(super) fn get_type_of_variable_access(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var_access: VariableAccess<'db>,
    ) -> Type<'db> {
        match var_access.kind(db) {
            VariableAccessKind::Symbolic(sym) => self.get_type_of_begin_path_expr(db, sym),
            VariableAccessKind::Direct(dv) => self
                .type_of_direct_variable
                .get(&dv)
                .copied()
                .unwrap_or_default(),
        }
    }

    pub fn type_of_begin_expr_with_adjustments(
        &self,
        db: &'db dyn WorkspaceDataBase,
        begin: BeginPathExpr<'db>,
    ) -> Type<'db> {
        match begin.expr(db) {
            Some(expr) => self.type_of_path_expr_with_adjustments(expr),
            None => match begin.invocation(db) {
                Some(invocation) => self
                    .type_of_invocation
                    .get(&invocation)
                    .copied()
                    .unwrap_or_default(),
                None => Default::default(),
            },
        }
    }

    pub(super) fn get_type_of_begin_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        begin: BeginPathExpr<'db>,
    ) -> Type<'db> {
        match begin.expr(db) {
            Some(expr) => self.get_type_of_path_expr(db, expr),
            None => match begin.invocation(db) {
                Some(invocation) => self.get_type_of_invocation(db, invocation),
                None => Default::default(),
            },
        }
    }

    pub fn get_type_of_invocation(
        &self,
        db: &'db dyn WorkspaceDataBase,
        invocation: Invocation<'db>,
    ) -> Type<'db> {
        self.type_of_invocation
            .get(&invocation)
            .copied()
            .unwrap_or_default()
    }

    pub fn type_of_path_expr_with_adjustments(&self, expr: PathExpr<'db>) -> Type<'db> {
        match self
            .path_expr_adjustments
            .get(&expr)
            .and_then(|adjustements| adjustements.last())
        {
            Some(adjustment) => adjustment.target,
            None => self
                .type_of_path_expr
                .get(&expr)
                .copied()
                .unwrap_or_default(),
        }
    }

    /// Check whether a variable access is rooted in a DataType (constant).
    /// The first resolved step in the path determines constness.
    /// Only applies to multi-step paths (e.g., TYPE_NAME.field) - bare type
    /// names are handled by check_not_direct_type.
    pub fn is_constant_access(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var_access: VariableAccess<'db>,
    ) -> bool {
        let VariableAccessKind::Symbolic(sym) = var_access.kind(db) else {
            return false;
        };
        let Some(path) = sym.expr(db) else {
            return false;
        };
        self.is_constant_path(db, path)
    }

    /// Check whether an expression is a DataType constant access.
    /// Returns true if the expr is a variable access rooted in a DataType.
    pub fn is_constant_type(&self, db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> bool {
        if let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) = expr.expr(db) {
            self.is_constant_access(db, *va)
        } else {
            false
        }
    }

    fn is_constant_path(&self, db: &'db dyn WorkspaceDataBase, path: PathExpr<'db>) -> bool {
        let steps = path.flatten(db);
        if steps.len() < 2 {
            return false;
        }
        matches!(
            self.type_of_path_expr.get(&steps[0].get_expr(db)),
            Some(Type::DataType(_))
        )
    }

    pub(super) fn get_type_of_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: PathExpr<'db>,
    ) -> Type<'db> {
        self.type_of_path_expr
            .get(&expr)
            .copied()
            .unwrap_or_default()
    }

    pub fn adjustments_of_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: Expr<'db>,
    ) -> Option<&[Adjustment<'db>]> {
        match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(var)) => {
                self.adjustments_of_var_access(db, *var)
            }
            ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                value: RefValue::Address(address),
            }) => self.adjustments_of_begin_path_expr(db, *address),
            _ => None,
        }
    }

    pub fn adjustments_of_var_access(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var: VariableAccess<'db>,
    ) -> Option<&[Adjustment<'db>]> {
        match var.kind(db) {
            VariableAccessKind::Symbolic(sym) => self.adjustments_of_begin_path_expr(db, sym),
            _ => None,
        }
    }

    pub fn adjustments_of_begin_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        begin: BeginPathExpr<'db>,
    ) -> Option<&[Adjustment<'db>]> {
        match begin.expr(db) {
            Some(expr) => self.adjustments_of_path_expr(expr),
            None => None,
        }
    }

    pub fn adjustments_of_path_expr(&self, expr: PathExpr<'db>) -> Option<&[Adjustment<'db>]> {
        self.path_expr_adjustments.get(&expr).map(|it| &**it)
    }

    pub fn variable_for_param(&self, param: ParamAssign<'db>) -> Option<VariableDecl<'db>> {
        self.variable_of_param.get(&param).copied()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Adjust {
    Deref,
    Ref,
    Index,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct Adjustment<'db> {
    pub kind: Adjust,
    pub target: Type<'db>,
}

impl<'db> Adjustment<'db> {
    pub fn new_deref(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Deref,
            target: ty,
        }
    }

    pub fn new_index(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Index,
            target: ty,
        }
    }

    pub fn new_ref(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Ref,
            target: ty,
        }
    }
}

pub trait AdjustmentInfo<'db> {
    fn as_reference(&self) -> Option<Type<'db>>;
    fn as_dereference(&self) -> Option<Type<'db>>;
    fn as_index(&self) -> Option<Type<'db>>;
    fn array_dimensions(&self, array_type: &Type<'db>) -> usize;
}

impl<'db> AdjustmentInfo<'db> for [Adjustment<'db>] {
    fn as_reference(&self) -> Option<Type<'db>> {
        if let Some(adj) = self.last()
            && adj.kind == Adjust::Ref
        {
            return Some(adj.target);
        }
        None
    }

    fn as_dereference(&self) -> Option<Type<'db>> {
        if let Some(adj) = self.last()
            && adj.kind == Adjust::Deref
        {
            return Some(adj.target);
        }
        None
    }

    fn as_index(&self) -> Option<Type<'db>> {
        if let Some(adj) = self.last()
            && adj.kind == Adjust::Index
        {
            return Some(adj.target);
        }
        None
    }

    fn array_dimensions(&self, array_type: &Type<'db>) -> usize {
        self.iter()
            .rev()
            .take_while(|adj| matches!(adj.kind, Adjust::Index) && adj.target.eq(array_type))
            .count()
    }
}
