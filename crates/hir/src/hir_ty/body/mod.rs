// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    CallSite, HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, ExprKind, FuncCall, InitExprKind, ParamAssign, PathExpr,
                PrimaryExpr, RefValue, VariableAccess, VariableAccessKind,
            },
            invocation::Invocation,
            statement::Stmt,
        },
        pous::{
            pou::Pou,
            variable::{DirectVariable, VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
        using::Using,
    },
    hir_ty::{
        body::statements::{NestedScope, StmtsResolverCtx},
        head::init_inference::infer_initialization,
        infer::Infer,
        resolver::Resolver,
        ty::Type,
    },
};

pub mod statements;

/// What inference knows about a scope: its statements' result and its
/// initializers'. A FUNCTION's locals start over at each call, before its
/// first statement, so an initializer's `p^`, `REF(v)`, `o => wide` or
/// `SUPER.m()` is the scope's as much as a statement's; read from the
/// statements' result alone, each of those compiled wrong or not at all.
/// The two stay separate queries, so an edit to a body leaves its
/// declarations alone, and this is the one way out of the crate to read
/// either: a lookup answers from whichever recorded the node, a scan walks
/// both.
///
/// A node is recorded by one of the two. The statements' result is asked
/// first; the initializers', memoized, is fetched on a miss.
#[derive(Clone, Copy)]
pub struct ScopeInference<'db> {
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    statements: &'db BodyInferenceResult<'db>,
}

impl<'db> ScopeId<'db> {
    /// What inference knows about this scope's statements and initializers.
    pub fn inference(self, db: &'db dyn WorkspaceDataBase) -> ScopeInference<'db> {
        ScopeInference {
            db,
            scope: self,
            statements: infer_body(db, self),
        }
    }
}

impl<'db> ScopeInference<'db> {
    fn initializers(self) -> &'db BodyInferenceResult<'db> {
        &infer_initialization(self.db, self.scope).body_infer_result
    }

    /// Both results, the statements' first.
    fn both(self) -> [&'db BodyInferenceResult<'db>; 2] {
        [self.statements, self.initializers()]
    }

    /// The answer of the result that recorded the node.
    fn recorded<T>(self, get: impl Fn(&'db BodyInferenceResult<'db>) -> Option<T>) -> Option<T> {
        get(self.statements).or_else(|| get(self.initializers()))
    }

    /// A recorded type, `Never` being the miss.
    fn typed(self, get: impl Fn(&'db BodyInferenceResult<'db>) -> Type<'db>) -> Type<'db> {
        let ty = get(self.statements);
        if ty.is_never() {
            get(self.initializers())
        } else {
            ty
        }
    }

    // One node.

    /// The type of an expression, before its adjustments.
    pub fn type_of_expr(self, expr: Expr<'db>) -> Type<'db> {
        self.typed(|r| r.get_type_of_expr(expr))
    }

    /// The type an expression evaluates to: `arr[0]` is the element, `r^`
    /// the target.
    pub fn type_of_expr_adjusted(self, expr: Expr<'db>) -> Type<'db> {
        self.typed(|r| r.type_of_expr_with_adjustments(self.db, expr))
    }

    pub fn type_of_path_expr(self, path: PathExpr<'db>) -> Type<'db> {
        self.typed(|r| r.get_type_of_path_expr(self.db, path))
    }

    pub fn type_of_path_expr_adjusted(self, path: PathExpr<'db>) -> Type<'db> {
        self.typed(|r| r.type_of_path_expr_with_adjustments(path))
    }

    pub fn type_of_begin_path_expr(self, begin: BeginPathExpr<'db>) -> Type<'db> {
        self.typed(|r| r.get_type_of_begin_path_expr(self.db, begin))
    }

    pub fn type_of_begin_path_expr_adjusted(self, begin: BeginPathExpr<'db>) -> Type<'db> {
        self.typed(|r| r.type_of_begin_expr_with_adjustments(self.db, begin))
    }

    pub fn type_of_variable_access(self, access: VariableAccess<'db>) -> Type<'db> {
        self.typed(|r| r.get_type_of_variable_access(self.db, access))
    }

    pub fn type_of_variable_access_adjusted(self, access: VariableAccess<'db>) -> Type<'db> {
        self.typed(|r| r.type_of_variable_access_with_adjustments(self.db, access))
    }

    pub fn type_of_invocation(self, invocation: Invocation<'db>) -> Type<'db> {
        self.typed(|r| r.get_type_of_invocation(self.db, invocation))
    }

    pub fn adjustments_of_path_expr(self, path: PathExpr<'db>) -> Option<&'db [Adjustment<'db>]> {
        self.recorded(|r| r.adjustments_of_path_expr(path))
    }

    /// What a bracket indexes, as the walk decided it.
    pub fn indexed_array(self, path: PathExpr<'db>) -> Option<IndexedArray<'db>> {
        self.recorded(|r| r.indexed_arrays.get(&path).copied())
    }

    /// The declaration a path step names, when it names a variable.
    pub fn variable_for_path_expr(self, path: PathExpr<'db>) -> Option<VariableDecl<'db>> {
        self.recorded(|r| r.variable_for_path_expr(path))
    }

    /// Whether a path step named a namespace rather than a value.
    pub fn path_expr_is_namespace(self, path: PathExpr<'db>) -> bool {
        self.both().iter().any(|r| r.path_expr_is_namespace(path))
    }

    /// The parameter an argument binds.
    pub fn variable_for_param(self, param: ParamAssign<'db>) -> Option<VariableDecl<'db>> {
        self.recorded(|r| r.variable_for_param(param))
    }

    /// An argument's 1-based position in the variadic pack it binds.
    pub fn variadic_position(self, param: ParamAssign<'db>) -> Option<usize> {
        self.recorded(|r| r.variadic_position.get(&param).copied())
    }

    /// The plan assembled for a call: its callee and what it binds to each
    /// parameter.
    pub fn resolved_call(self, call: FuncCall<'db>) -> Option<&'db ResolvedCall<'db>> {
        self.recorded(|r| r.resolved_calls.get(&call))
    }

    /// The type the site consuming a value converts it to.
    pub fn coercion_target(self, expr: Expr<'db>) -> Option<Type<'db>> {
        self.recorded(|r| r.coercion_target.get(&expr).copied())
    }

    /// The type a comparison's operands are compared at.
    pub fn comparison_operand_type(self, expr: Expr<'db>) -> Option<Type<'db>> {
        self.recorded(|r| r.comparison_operand_type.get(&expr).copied())
    }

    // Every node, of the statements and then of the initializers.

    /// Every call, in resolution order.
    pub fn calls(self) -> impl Iterator<Item = FuncCall<'db>> {
        let [a, b] = self.both();
        a.calls.iter().chain(&b.calls).copied()
    }

    pub fn resolved_calls(self) -> impl Iterator<Item = (FuncCall<'db>, &'db ResolvedCall<'db>)> {
        let [a, b] = self.both();
        a.resolved_calls
            .iter()
            .chain(&b.resolved_calls)
            .map(|(call, resolved)| (*call, resolved))
    }

    pub fn invocations(self) -> impl Iterator<Item = Invocation<'db>> {
        let [a, b] = self.both();
        a.type_of_invocation
            .keys()
            .chain(b.type_of_invocation.keys())
            .copied()
    }

    /// Every expression with its type, before adjustments.
    pub fn typed_exprs(self) -> impl Iterator<Item = (Expr<'db>, Type<'db>)> {
        let [a, b] = self.both();
        a.type_of_expr
            .iter()
            .chain(&b.type_of_expr)
            .map(|(expr, ty)| (*expr, *ty))
    }

    pub fn typed_path_exprs(self) -> impl Iterator<Item = (PathExpr<'db>, Type<'db>)> {
        let [a, b] = self.both();
        a.type_of_path_expr
            .iter()
            .chain(&b.type_of_path_expr)
            .map(|(path, ty)| (*path, *ty))
    }

    pub fn variables_used(self) -> impl Iterator<Item = VariableDecl<'db>> {
        let [a, b] = self.both();
        a.variables_used.iter().chain(&b.variables_used).copied()
    }

    pub fn usings_used(self) -> impl Iterator<Item = Using<'db>> {
        let [a, b] = self.both();
        a.usings_used.iter().chain(&b.usings_used).copied()
    }

    /// Each variable that shadows a POU of its name, with the POU.
    pub fn variables_shadowing(self) -> impl Iterator<Item = (VariableDecl<'db>, Pou<'db>)> {
        let [a, b] = self.both();
        a.variables_shadowing
            .iter()
            .chain(&b.variables_shadowing)
            .map(|(var, pou)| (*var, *pou))
    }

    /// Each method variable that shadows a member of the owner, with the
    /// member.
    pub fn method_shadowed_members(
        self,
    ) -> impl Iterator<Item = (VariableDecl<'db>, VariableDecl<'db>)> {
        let [a, b] = self.both();
        a.method_shadowed_members
            .iter()
            .chain(&b.method_shadowed_members)
            .map(|(var, member)| (*var, *member))
    }

    /// Each access to a global with no VAR_EXTERNAL for it, with the global.
    pub fn globals_without_external(
        self,
    ) -> impl Iterator<Item = (PathExpr<'db>, VariableDecl<'db>)> {
        let [a, b] = self.both();
        a.globals_without_external
            .iter()
            .chain(&b.globals_without_external)
            .copied()
    }

    /// Whether `var` is the FUNCTION's or METHOD's result, or a part of it.
    pub fn writes_result(self, var: VariableAccess<'db>) -> bool {
        self.both().iter().any(|r| r.writes_result(self.db, var))
    }

    /// Whether the result is handed to something that can write it: an
    /// output, a VAR_IN_OUT or a `REF()`.
    pub fn hands_out_result(self) -> bool {
        self.both().iter().any(|r| r.hands_out_result(self.db))
    }

    // Statements only: an initializer has none.

    /// The value of a CASE label, as inference evaluated it.
    pub fn case_label_value(self, label: Expr<'db>) -> Option<&'db CaseLabelValue> {
        self.statements.case_label_value.get(&label)
    }

    /// The folded value of a FOR step.
    pub fn for_step_value(self, step: Expr<'db>) -> Option<i64> {
        self.statements.for_step_value.get(&step).copied()
    }

    /// The first `SUPER()` of a function block body.
    pub fn first_super_body(self) -> Option<Stmt<'db>> {
        self.statements.first_super_body
    }

    pub fn unused_return_types(self) -> &'db [(Stmt<'db>, Type<'db>)] {
        &self.statements.unused_return_types
    }

    pub fn effectless_statements(self) -> &'db [Stmt<'db>] {
        &self.statements.effectless_statements
    }

    pub fn case_without_else(self) -> &'db [Stmt<'db>] {
        &self.statements.case_without_else
    }

    pub fn dead_code_statements(self) -> &'db [Stmt<'db>] {
        &self.statements.dead_code_statements
    }

    pub fn mismatched_for_step(self) -> &'db [Stmt<'db>] {
        &self.statements.mismatched_for_step
    }
}

#[tracing::instrument(level = "trace", skip(db))]
#[salsa::tracked(returns(ref))]
pub(crate) fn infer_body<'db>(
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
        // No statements, but paths that name variables.
        ScopeKind::Config(config) => {
            crate::hir_ty::config::record_config_paths(db, config, &mut result);
            return result;
        }
        _ => return result,
    };

    // Initialize null state tracking for REF_TO local variables
    init_ref_null_states(db, scope, &mut result);

    // Record method locals/params that shadow an owner FB/Class member.
    init_method_member_shadows(db, scope, &mut result);

    let resolver = Resolver::for_scope(db, scope);

    ctx.check_statements(db, resolver, statements, NestedScope::None, &mut result);

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
            let state = match var.init(db) {
                Some(init) => {
                    if is_null_init(db, &init.kind(db)) {
                        NullState::Null(NullOrigin {
                            site: init.as_call_site(db),
                            var: *var,
                        })
                    } else {
                        NullState::NonNull
                    }
                }
                // The declaration is its name: that is what the report points at.
                None => NullState::Uninitialized(NullOrigin {
                    site: CallSite::new(var.get_scope_id(db), crate::HasName::get_name_id(var, db)),
                    var: *var,
                }),
            };
            result.ref_null_state.insert(*var, state);
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

/// Where a nullable state came from: the source location, and the variable
/// that location declares or assigns.
///
/// Both travel together because a state PROPAGATES (`ptr := ptr_2` copies
/// ptr_2's state to ptr), so the site may belong to a different variable than
/// the one being dereferenced. Carrying only the site named the dereferenced
/// variable against another variable's span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub struct NullOrigin<'db> {
    pub site: CallSite<'db>,
    pub var: VariableDecl<'db>,
}

/// Nullability state for REF_TO variables, tracked during linear statement flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum NullState<'db> {
    /// Variable was never assigned (default for REF_TO without initializer).
    /// The origin points to the variable declaration.
    Uninitialized(NullOrigin<'db>),
    /// Variable was explicitly assigned NULL.
    /// The origin points to the NULL assignment or NULL initializer.
    Null(NullOrigin<'db>),
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
            (s @ NullState::Null(..), _) | (_, s @ NullState::Null(..)) => s,
            (s @ NullState::Uninitialized(..), _) | (_, s @ NullState::Uninitialized(..)) => s,
        }
    }
}

/// Result of body inference
///
/// When the this struct is emitted via the [`infer_body`] query, it is important to note 2 things about the type mappings:
///
/// 1. The types mapped to expressions and invocations are the types *before* any normalization or adjustments are applied.
///    see the note on normalization in the normalize module.
///
/// 2. There should be no [`Type::Infer`] types in the mappings. All types should be fully resolved,
///    those that can't be resolved will be represented as [`Type::Never`].

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct BodyInferenceResult<'db> {
    // Scope where this InferenceResult was emitted
    pub(crate) scope: ScopeId<'db>,

    // Mapping from parameter assignments to variables
    pub(crate) variable_of_param: FxHashMap<ParamAssign<'db>, VariableDecl<'db>>,

    /// The assembled plan for each call: the resolved callee and, per
    /// declared parameter IN DECLARATION ORDER, what the call binds to it.
    /// A consumer that assembles the call again from the raw assigns
    /// re-decides matching, ordering and defaults — a mismatch is a
    /// positional-argument shift in the emitted call.
    pub(crate) resolved_calls: FxHashMap<FuncCall<'db>, ResolvedCall<'db>>,

    // For variadic parameters, stores the 1-based position index
    pub(crate) variadic_position: FxHashMap<ParamAssign<'db>, usize>,

    // Mapping of direct variables to their types
    pub(crate) type_of_direct_variable: FxHashMap<DirectVariable<'db>, Type<'db>>,

    // Mapping from invocations to their resolved types.
    pub(crate) type_of_invocation: FxHashMap<Invocation<'db>, Type<'db>>,

    // Mapping from path expressions to their resolved types.
    pub(crate) type_of_path_expr: FxHashMap<PathExpr<'db>, Type<'db>>,

    /// The declaration each path step resolved to, for the steps that name a
    /// variable. Kept beside the type because the type does not survive: a
    /// callee path is re-typed as its `CallableType` once the call resolves,
    /// so `motor()` ends up typed as the block it invokes. Consumers that
    /// need the DECLARATION — the instance a call runs on, the identity
    /// rename and references work from — read it here instead of resolving
    /// the name a second time.
    pub(crate) variable_of_path_expr: FxHashMap<PathExpr<'db>, VariableDecl<'db>>,

    /// The path steps that named a NAMESPACE on the way to a fully-qualified
    /// item. A namespace is not a value, so it has no type to record, and
    /// without this the `Std` in `Std.Convert.X` is indistinguishable from a
    /// name that did not resolve at all.
    pub(crate) namespace_of_path_expr: FxHashSet<PathExpr<'db>>,

    // Mapping from expressions to their resolved types.
    pub(crate) type_of_expr: FxHashMap<Expr<'db>, Type<'db>>,

    // For a comparison expression, the common type its OPERANDS are compared
    // at — their join in the implicit-widening lattice.
    //
    // `type_of_expr` for a comparison is BOOL (its result), which loses the
    // operand type that codegen needs in order to pick the machine comparison
    // and to insert operand casts. Recording it here keeps that decision in
    // inference, where the widening lattice lives, instead of leaving each
    // consumer to re-derive it.
    pub(crate) comparison_operand_type: FxHashMap<Expr<'db>, Type<'db>>,

    /// The folded value of each FOR step expression the check accepted. The
    /// step's SIGN picks the loop's exit comparison at compile time, so a
    /// step that does not fold is refused (E1204) — silently treating it as
    /// ascending ran a `BY n` loop with `n = -1` zero times.
    pub(crate) for_step_value: FxHashMap<Expr<'db>, i64>,

    /// Every call resolution visited, in resolution order. A consumer that
    /// needs "all calls in this body" reads this instead of re-walking the
    /// statement tree for the shapes a call can hide in.
    pub(crate) calls: Vec<FuncCall<'db>>,

    /// The type a value is converted to by the site that consumes it — an
    /// assignment target, a parameter, an FB input. Inference decides this
    /// when it checks the coercion; recording it keeps a consumer that has to
    /// emit the conversion from deciding a second time, which is how an
    /// assignment and a call argument came to disagree about the same pair of
    /// types.
    pub(crate) coercion_target: FxHashMap<Expr<'db>, Type<'db>>,

    // The value of each CASE label, evaluated here.
    //
    // IEC's `Case_List_Elem : Subrange | Constant_Expr`, and a constant
    // expression is any expression that evaluates at compile time — a named
    // CONSTANT and arithmetic over constants included. Checking is therefore
    // the same act as evaluating, so the value is recorded rather than left
    // for the consumer to work out again: lowering reads THIS instead of
    // lowering the label and inspecting what came out, which is how a label
    // it could not fold became an internal compiler error.
    pub(crate) case_label_value: FxHashMap<Expr<'db>, CaseLabelValue>,

    // Mapping from path expressions to their adjustment sequences.
    pub(crate) path_expr_adjustments: FxHashMap<PathExpr<'db>, Vec<Adjustment<'db>>>,

    /// What each bracket (`a[i]`) indexes, as the walk decided it: the
    /// bounds check and MIR read it here rather than work it out again.
    pub(crate) indexed_arrays: FxHashMap<PathExpr<'db>, IndexedArray<'db>>,

    // Errors encountered during inference
    pub(crate) errors: Vec<IdeDiagnostic>,

    // Set of variables that were referenced in the body.
    // Populated during statement resolution for use by the linter.
    pub(crate) variables_used: FxHashSet<VariableDecl<'db>>,

    // Set of USING directives that were actually used to resolve a name.
    // Populated during name resolution for use by the linter.
    pub(crate) usings_used: FxHashSet<Using<'db>>,

    // Variables that shadow a POU with the same name.
    // Populated during statement resolution for use by the linter.
    pub(crate) variables_shadowing: FxHashMap<VariableDecl<'db>, Pou<'db>>,

    // Method locals/params that shadow a member of the owner FB/Class (same
    // name). Maps the method variable → the shadowed member. Populated for
    // method bodies for use by the linter.
    pub(crate) method_shadowed_members: FxHashMap<VariableDecl<'db>, VariableDecl<'db>>,

    // Config/resource VAR_GLOBALs accessed directly by name without a matching
    // VAR_EXTERNAL declaration (direct access). Resolution still
    // succeeds; the linter warns, since strict IEC wants an explicit
    // VAR_EXTERNAL. Records (access path expr, resolved global decl).
    pub(crate) globals_without_external: Vec<(PathExpr<'db>, VariableDecl<'db>)>,

    // Function calls whose return value is discarded.
    // Populated during statement resolution for use by the linter.
    pub(crate) unused_return_types: Vec<(Stmt<'db>, Type<'db>)>,

    // Statements that are just expressions with no side effects.
    // Populated during statement resolution for use by the linter.
    pub(crate) effectless_statements: Vec<Stmt<'db>>,

    // CASE statements without an ELSE clause.
    // Populated during statement resolution for use by the linter.
    pub(crate) case_without_else: Vec<Stmt<'db>>,

    // Statements that are unreachable (after RETURN/EXIT/CONTINUE).
    // Populated during statement resolution for use by the linter.
    pub(crate) dead_code_statements: Vec<Stmt<'db>>,

    // FOR loops where step sign mismatches bounds direction.
    // Populated during statement resolution for use by the linter.
    pub(crate) mismatched_for_step: Vec<Stmt<'db>>,

    // Null state tracking for REF_TO variables.
    // Tracks whether a reference variable is initialized / null / non-null
    // through linear statement flow.
    pub(crate) ref_null_state: FxHashMap<VariableDecl<'db>, NullState<'db>>,

    // The first `SUPER()` (base-body call) statement seen in a function block
    // body. Per IEC 6.6.7.2.9 rule 2, `SUPER()` shall occur once — a second
    // occurrence is reported (E1110), pointing back to this first one.
    pub(crate) first_super_body: Option<Stmt<'db>>,
}

impl<'db> BodyInferenceResult<'db> {
    pub(crate) fn new(scope: ScopeId<'db>) -> Self {
        Self {
            scope,
            variable_of_param: FxHashMap::default(),
            resolved_calls: FxHashMap::default(),
            variadic_position: FxHashMap::default(),
            type_of_direct_variable: FxHashMap::default(),
            type_of_invocation: FxHashMap::default(),
            type_of_expr: FxHashMap::default(),
            comparison_operand_type: FxHashMap::default(),
            for_step_value: FxHashMap::default(),
            calls: Vec::new(),
            coercion_target: FxHashMap::default(),
            case_label_value: FxHashMap::default(),
            type_of_path_expr: FxHashMap::default(),
            variable_of_path_expr: FxHashMap::default(),
            namespace_of_path_expr: FxHashSet::default(),
            path_expr_adjustments: FxHashMap::default(),
            indexed_arrays: FxHashMap::default(),
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
            first_super_body: None,
        }
    }

    /// E0704 for a reference bound from `REF(x)` or from another reference:
    /// the pointee and the referenced variable must agree about the
    /// subrange, exactly as a VAR_IN_OUT must with its argument. `REF_TO
    /// INT := REF(s)` with `s : INT (0..10)` let `p^ := 500` go around the
    /// range check entirely. A differing BASE is E0301's complaint, not this.
    pub(crate) fn ref_subrange_mismatch(
        &self,
        db: &'db dyn WorkspaceDataBase,
        target: Type<'db>,
        value: Expr<'db>,
    ) -> Option<crate::check::errors::e07_subrange::SubRangeError<'db>> {
        use crate::HirNodeInfo;
        use crate::hir_def::expressions::expression::{ExprKind, PrimaryExpr, RefValue};
        let Type::RefTo(spec) = target.normalize(db) else {
            return None;
        };
        let pointee = spec.infer(db);
        let value_ty = self.type_of_expr.get(&value).copied()?;
        // `REF(x)` records x's own type; a reference-typed value points at
        // its pointee.
        let referenced = match value.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                value: RefValue::Address(_),
            }) => value_ty,
            _ => match value_ty.normalize(db) {
                Type::RefTo(s) => s.infer(db),
                _ => return None,
            },
        };
        if pointee.normalize(db) != referenced.normalize(db) {
            return None;
        }
        let bounds = |t: Type<'db>| {
            t.as_subrange(db)
                .map(|s| crate::hir_ty::infer::const_eval::subrange_bounds(db, s))
        };
        let agree = match (bounds(pointee), bounds(referenced)) {
            (None, None) => true,
            (Some(p), Some(a)) => p == a,
            _ => false,
        };
        (!agree).then(
            || crate::check::errors::e07_subrange::SubRangeError::ByRefSubrangeMismatch {
                span: value.get_span(db),
                param: pointee,
                arg: referenced,
            },
        )
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
            // A name read where a value belongs but naming none, a type or a
            // FUNCTION, was reported there (E0317) and typed Never.
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(_))
                if self.type_of_expr.get(&expr).is_some_and(Type::is_never) =>
            {
                Type::Never
            }
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

    /// Whether an argument names a place inside a constant: see
    /// [`Self::is_constant_place_path`].
    pub fn is_constant_value(&self, db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> bool {
        match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) => {
                self.is_constant_place(db, *va)
            }
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
                self.is_constant_value(db, *expr)
            }
            _ => false,
        }
    }

    /// Whether a store into the place `access` names changes a constant:
    /// see [`Self::is_constant_place_path`].
    pub fn is_constant_place(
        &self,
        db: &'db dyn WorkspaceDataBase,
        access: VariableAccess<'db>,
    ) -> bool {
        let VariableAccessKind::Symbolic(sym) = access.kind(db) else {
            return false;
        };
        sym.expr(db)
            .is_some_and(|path| self.is_constant_place_path(db, path))
    }

    /// Whether a store into the place `path` names changes a constant: the
    /// path is rooted in a TYPE, or passes through a CONSTANT variable. An
    /// element or a field of a constant is the constant, and so is what a
    /// constant reference points to, as an assignment through one is refused.
    pub fn is_constant_place_path(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path: PathExpr<'db>,
    ) -> bool {
        self.is_constant_path(db, path) || self.constant_root_path(db, path).is_some()
    }

    /// The CONSTANT variable a place passes through: `p` for `p.x`.
    pub fn constant_root_path(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path: PathExpr<'db>,
    ) -> Option<VariableDecl<'db>> {
        use crate::hir_ty::expr_store::PathExprWalkStep;
        path.flatten(db).iter().find_map(|step| {
            match (step, self.type_of_path_expr.get(&step.get_expr(db))) {
                (PathExprWalkStep::Field { .. }, Some(Type::Variable((var, _))))
                    if var.qualifier(db).contains(crate::Qualifier::CONSTANT) =>
                {
                    Some(*var)
                }
                _ => None,
            }
        })
    }

    /// [`Self::constant_root_path`] of an access.
    pub fn constant_root(
        &self,
        db: &'db dyn WorkspaceDataBase,
        access: VariableAccess<'db>,
    ) -> Option<VariableDecl<'db>> {
        let VariableAccessKind::Symbolic(sym) = access.kind(db) else {
            return None;
        };
        self.constant_root_path(db, sym.expr(db)?)
    }

    /// [`Self::constant_root_path`] of an argument.
    pub fn constant_root_value(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: Expr<'db>,
    ) -> Option<VariableDecl<'db>> {
        match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) => self.constant_root(db, *va),
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
                self.constant_root_value(db, *expr)
            }
            _ => None,
        }
    }

    /// A multi-step path rooted in a DataType (`TYPE_NAME.field`): a bare
    /// type name is handled by `check_not_direct_type`.
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

    /// Whether `var` is the enclosing FUNCTION's or METHOD's return value, or
    /// a part of it: `Compute := 1`, `MakePt.x := 3`.
    pub fn writes_result(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var: crate::hir_def::expressions::expression::VariableAccess<'db>,
    ) -> bool {
        use crate::hir_def::expressions::expression::VariableAccessKind;
        let VariableAccessKind::Symbolic(begin) = var.kind(db) else {
            return false;
        };
        self.names_result(db, begin)
    }

    /// Whether `path` starts at the enclosing FUNCTION's or METHOD's return
    /// value.
    pub fn names_result(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path: crate::hir_def::expressions::expression::BeginPathExpr<'db>,
    ) -> bool {
        path.expr(db)
            .and_then(|path| path.flatten(db).first().map(|step| step.get_expr(db)))
            .is_some_and(|root| {
                matches!(
                    self.type_of_path_expr.get(&root),
                    Some(Type::ReturnValue(_))
                )
            })
    }

    /// Whether the return value is handed to something that can write it:
    /// bound to an output (`o => F`), passed to a VAR_IN_OUT, or referenced
    /// with `REF(F)`.
    pub fn hands_out_result(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        use crate::hir_def::expressions::expression::{
            ExprKind, ParamAssignKind, PrimaryExpr, RefValue,
        };
        let bound = self
            .variable_of_param
            .iter()
            .any(|(param, var)| match param.kind(db) {
                ParamAssignKind::FormalOutput { variable, .. } => self.writes_result(db, variable),
                ParamAssignKind::NonFormal { value }
                | ParamAssignKind::FormalInput { value, .. } => {
                    var.is_in_out(db)
                        && matches!(
                            value.expr(db),
                            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access))
                                if self.writes_result(db, *access)
                        )
                }
            });
        bound
            || self.type_of_expr.keys().any(|expr| {
                matches!(
                    expr.expr(db),
                    ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                        value: RefValue::Address(path),
                    }) if self.names_result(db, *path)
                )
            })
    }

    /// The declaration this path step names, when it names a variable.
    pub fn variable_for_path_expr(&self, expr: PathExpr<'db>) -> Option<VariableDecl<'db>> {
        self.variable_of_path_expr.get(&expr).copied()
    }

    /// Whether this path step named a namespace rather than a value.
    pub fn path_expr_is_namespace(&self, expr: PathExpr<'db>) -> bool {
        self.namespace_of_path_expr.contains(&expr)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Adjust {
    Deref,
    Ref,
    Index,
}

/// The array a bracket indexes, and the dimensions of it the brackets before
/// consumed and this one does. A bracket after one that left dimensions of a
/// multi-dimensional array continues that array, `m[i][j]`; any other
/// indexes the element it reached from its first dimension, `r[i][j]` of an
/// array of rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct IndexedArray<'db> {
    pub array: crate::hir_def::expressions::spec::Array<'db>,
    /// The dimensions consumed before this bracket's subscripts.
    pub first: usize,
    /// The dimensions consumed once they are.
    pub through: usize,
}

impl<'db> IndexedArray<'db> {
    /// Whether the bracket leaves dimensions of the array to a further one:
    /// `m[i]` of a 2-D `m` names a part of it, and is no value.
    pub fn is_partial(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.through < self.array.subranges(db).len()
    }
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
}

/// The plan resolution assembled for one call — see
/// [`BodyInferenceResult::resolved_calls`].
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolvedCall<'db> {
    pub callable: crate::hir_ty::ty::CallableType<'db>,
    /// One entry per declared parameter (inputs, outputs, inouts), in the
    /// callee's declaration order.
    pub params: Vec<(VariableDecl<'db>, ParamBinding<'db>)>,
}

/// What a call site binds to one declared parameter.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ParamBinding<'db> {
    /// Input value(s) the site supplied (`:=` or positional). A variadic
    /// parameter collects several, in call order; anything else has one.
    Values(Vec<Expr<'db>>),
    /// `param => dest`.
    Output(VariableAccess<'db>),
    /// An omitted input, with the constant default that makes the omission
    /// legal (FUNCTION/METHOD only).
    Default(Expr<'db>),
    /// Nothing bound: an FB input keeps its instance storage, a discarded
    /// output receives whatever the ABI decides.
    Omitted,
}

/// A CASE label's compile-time value, in the domain it belongs to.
///
/// Labels come in two: integers (literals, CONSTANTs, arithmetic, and enum
/// ordinals) and strings. Keeping them in ONE map with the domain explicit is
/// what lets every consumer — lowering, the duplicate lint — ask the same
/// question and get an answer it cannot misread as the other kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum CaseLabelValue {
    Int(i128),
    /// The decoded bytes, so `STRING#'a'` and `'a'` are one label.
    Str(Vec<u8>),
}
