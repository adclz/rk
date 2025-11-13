use ast::generated::{PrimaryExpression, Subrange};
use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    HirNodeInfo,
    check::errors::path_error::AccessError,
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, ExprKind, FuncCall, InitExpr, ParamAssign, ParamAssignKind,
                PathExpr, PrimaryExpr, VariableAccess, VariableAccessKind,
            },
            invocation::{self, Invocation, InvocationKind},
            spec::{Array, ElementarySpec, Enum, Spec, SpecKind, Struct, StructElement, SubRange},
            statement::{Stmt, StmtKind},
        },
        pous::{
            class::{Class, MethodDecl},
            function::Function,
            function_block::FunctionBlock,
            interface::{Interface, MethodPrototype},
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::{HirNode, get_scope},
    },
    hir_ty::{
        def_map::LocalDefMap, flatten::{self, Flatten, PathExprWalkStep}, inference::{Adjustment, InferCtx, InferenceResult}, inheritance_solver::MethodRef, name_res::{pou_names_res, resolve_namespace_access}, ty::Ty, ty_var_access_resolver::CallSite
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Type<'db> {
    // Primitive types
    Elementary(ElementarySpec),
    RefTo(Spec<'db>),
    // Structured types
    Struct(Struct<'db>),
    StructElement(StructElement<'db>),
    Array(Array<'db>),
    ArrayConformand(Spec<'db>),
    Enum(Enum<'db>),
    SubRange(SubRange<'db>),
    // Pous
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),
    // Methods
    MethodDecl(MethodRef<'db>),
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CallableType<'db> {
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    MethodDecl(MethodRef<'db>),
}

impl<'db> CallableType<'db> {
    pub fn def_map(&self, db: &'db dyn BaseDatabase) -> &'db LocalDefMap<'db> {
        match self {
            CallableType::Function(f) => f.scope_id(db).def_map(db),
            CallableType::FunctionBlock(fb) => fb.scope_id(db).def_map(db),
            CallableType::MethodDecl(m) => m.get_scope_id(db).def_map(db),
        }
    }
}

impl<'db> Type<'db> {
    pub fn new_spec(db: &'db dyn BaseDatabase, spec: Spec<'db>) -> Self {
        match spec.kind(db) {
            SpecKind::Simple(elem) => Type::Elementary(*elem),
            SpecKind::Ref(ref_to) => Type::RefTo(*ref_to),
            SpecKind::Struct(strukt) => Type::Struct(*strukt),
            SpecKind::Array(arr) => Type::Array(*arr),
            SpecKind::ArrayConformand(a) => Type::ArrayConformand(*a),
            SpecKind::Enum(enm) => Type::Enum(*enm),
            SpecKind::Subrange(sub) => Type::SubRange(*sub),
            SpecKind::Target(t) => match resolve_namespace_access(db, &t.path) {
                Some(pou) => Type::new_pou(db, pou),
                None => Type::Never,
            },
        }
    }

    pub fn new_pou(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Self {
        match pou.pou(db) {
            Pou::Class(cl) => Type::Class(*cl),
            Pou::Function(f) => Type::Function(*f),
            Pou::FunctionBlock(f) => Type::FunctionBlock(*f),
            Pou::Interface(f) => Type::Interface(*f),
            Pou::DataType(dt) => Type::new_spec(db, dt.spec(db)),
        }
    }

    pub fn as_callable(&self) -> Option<CallableType<'db>> {
        Some(match self {
            Type::Function(f) => CallableType::Function(*f),
            Type::FunctionBlock(fb) => CallableType::FunctionBlock(*fb),
            Type::MethodDecl(m) => CallableType::MethodDecl(*m),
            _ => None?,
        })
    }

    pub fn walk_variable_access(
        &self,
        db: &'db dyn BaseDatabase,
        var_access: VariableAccess<'db>,
        ctx: &mut InferenceResult<'db>,
    ) {
        match var_access.kind(db) {
            VariableAccessKind::Direct { .. } => todo!(),
            VariableAccessKind::Symbolic(s) => self.walk_begin_path_expr(db, s, ctx),
        }
    }

    pub fn walk_begin_path_expr(
        &self,
        db: &'db dyn BaseDatabase,
        expr: BeginPathExpr<'db>,
        ctx: &mut InferenceResult<'db>,
    ) {
        let (typ, path) = if let Some(invocation) = expr.invocation(db) {
            InferCtx::resolve_invocation(db, ctx.scope, invocation, ctx);

            ctx.type_of_invocation
                .get(&invocation)
                .copied()
                .map(|t| (t, expr.expr(db)))
                .unwrap_or_else(|| (*self, expr.expr(db)))
        } else {
            (*self, expr.expr(db))
        };

        if let Some(path) = path {
            for step in path.flatten(db) {
                typ.walk_path_expr(db, step, ctx);
            }
        }
    }

    pub fn walk_path_expr(
        &self,
        db: &'db dyn BaseDatabase,
        step: &'db PathExprWalkStep<'db>,
        ctx: &mut InferenceResult<'db>,
    ) {
        match step {
            PathExprWalkStep::Deref { target, expr } => {
                match self {
                    Type::RefTo(ref_to) => {
                        ctx.type_of_path_expr.insert(*expr, *self);
                        ctx.adjustements.insert(
                            *expr,
                            vec![Adjustment::new_deref(db, Type::new_spec(db, *ref_to))]
                                .into_boxed_slice(),
                        );
                    }
                    _ => {
                        // Error: cannot deref non-ref type
                    }
                }
            }
            PathExprWalkStep::Field { ident, expr } => {
                match self {
                    Type::Function(_) | Type::FunctionBlock(_) | Type::Class(_) => {
                        let def_map = ctx.scope.def_map(db);

                        // Lookup variables first
                        def_map
                            .global_variables
                            .get(&ident.ident)
                            .map(|var| {
                                ctx.type_of_path_expr
                                    .insert(*expr, Type::new_spec(db, var.spec(db)));
                                ctx.variable_of_path_expr.insert(*expr, *var);
                            })
                            // Lookup Pous in scope
                            .or_else(|| {
                                pou_names_res(db, ident.ident, ctx.scope).map(|pou_decl| {
                                    ctx.type_of_path_expr
                                        .insert(*expr, Type::new_pou(db, pou_decl));
                                })
                            });
                    }
                    Type::Struct(st) => {
                        st.resolve_elements(db)
                            .get(&ident.ident)
                            .map(|s| ctx.type_of_path_expr.insert(*expr, Type::StructElement(*s)));
                    }
                    _ => {
                        // Error: cannot access field on non-structured type
                    }
                }
            }
            PathExprWalkStep::Index { expr } => {
                match self {
                    Type::Array(arr) => {
                        ctx.type_of_path_expr.insert(*expr, *self);
                        ctx.adjustements.insert(
                            *expr,
                            vec![Adjustment::new_index(
                                db,
                                Type::new_spec(db, arr.of_type(db)),
                            )]
                            .into_boxed_slice(),
                        );
                    }
                    _ => {
                        // Error: cannot index non-array type
                    }
                }
            }
        }
    }
}