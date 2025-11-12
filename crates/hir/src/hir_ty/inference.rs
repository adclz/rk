use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    HirNodeInfo,
    check::errors::path_error::AccessError,
    hir_def::{
        expressions::{
            expression::{Expr, InitExpr, PathExpr},
            spec::{Array, ElementarySpec, Spec, Struct},
        },
        pous::{
            class::{Class, MethodDecl},
            function::Function,
            function_block::FunctionBlock,
            interface::{Interface, MethodPrototype},
        },
        scope::ScopeId,
        semantic_index::HirNode,
    },
    hir_ty::{flatten::PathExprWalkStep, ty::Ty},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct Ty2<'db> {
    pub kind: TyKind2<'db>,
    pub scope_id: ScopeId<'db>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyKind2<'db> {
    // Primitive types
    Elementary(ElementarySpec),
    RefTo(Spec<'db>),
    // Structured types
    Struct(Struct<'db>),
    Array(Array<'db>),
    // Pous
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),
    // Methods
    MethodDecl(MethodDecl<'db>),
    MethodProt(MethodPrototype<'db>),
}

impl<'db> Ty2<'db> {
    pub fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        step: PathExprWalkStep<'db>,
        ctx: &mut InferenceResult<'db>,
    ) {
        match step {
            PathExprWalkStep::Deref { target, expr } => {
                match self.kind {
                    TyKind2::RefTo(ref_to) => {
                        ctx.type_of_path_expr.insert(expr, self.kind);
                        ctx.adjustements.insert(
                            expr,
                            vec![Adjustment::new_deref(db, ref_to)].into_boxed_slice(),
                        );
                    }
                    _ => {
                        // Error: cannot deref non-ref type
                    }
                }
            }
            PathExprWalkStep::Field { ident, expr } => match self.kind {
                TyKind2::Function(_) | TyKind2::FunctionBlock(_) | TyKind2::Class(_) => {
                    self.scope_id
                        .def_map(db)
                        .global_variables
                        .get(&ident.ident)
                        .map(|var| {
                            ctx.type_of_path_expr.insert(expr, var.ty(db).kind.clone());
                        });
                },
                _ => {
                    // Error: cannot access field on non-structured type
                }
            },
            PathExprWalkStep::Index { .. } => {
                match self.kind {
                    TyKind2::Array(arr) => {
                        ctx.type_of_path_expr.insert(expr, arr.element_ty(db).kind.clone());
                        ctx.adjustements.insert(
                            expr,
                            vec![Adjustment::new_index(db, arr.element_ty(db).kind.clone())]
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Adjust {
    Deref,
    Ref,
    Index,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Adjustment<'db> {
    pub kind: Adjust,
    pub target: TyKind2<'db>,
}

impl<'db> Adjustment<'db> {
    pub fn new_deref(db: &'db dyn BaseDatabase, ty: TyKind2<'db>) -> Self {
        Adjustment {
            kind: Adjust::Deref,
            target: ty,
        }
    }

    pub fn new_ref(db: &'db dyn BaseDatabase, ty: TyKind2<'db>) -> Self {
        Adjustment {
            kind: Adjust::Ref,
            target: ty,
        }
    }

    pub fn new_index(db: &'db dyn BaseDatabase, ty: TyKind2<'db>) -> Self {
        Adjustment {
            kind: Adjust::Index,
            target: ty,
        }
    }
}

pub struct InferenceResult<'db> {
    // Mapping from path expressions to their resolved types.
    pub type_of_path_expr: FxHashMap<PathExpr<'db>, TyKind2<'db>>,

    // Mapping from expressions to their resolved types.
    pub type_of_expr: FxHashMap<Expr<'db>, TyKind2<'db>>,

    // Mapping from initialization expressions to their resolved types.
    pub type_of_init_expr: FxHashMap<InitExpr<'db>, TyKind2<'db>>,

    // Mapping from path expressions to their adjustment sequences.
    pub adjustements: FxHashMap<PathExpr<'db>, Box<[Adjustment<'db>]>>,

    pub errors: Vec<AccessError<'db>>,
}

impl<'db> InferenceResult<'db> {
    pub fn new() -> Self {
        InferenceResult {
            type_of_expr: FxHashMap::default(),
            type_of_path_expr: FxHashMap::default(),
            type_of_init_expr: FxHashMap::default(),
            adjustements: FxHashMap::default(),
            errors: Vec::new(),
        }
    }

    pub fn type_of_path_with_adjustments(&self, expr: PathExpr<'db>) -> Option<TyKind2<'db>> {
        match self
            .adjustements
            .get(&expr)
            .and_then(|adjustements| adjustements.last())
        {
            Some(adjustment) => Some(adjustment.target.clone()),
            None => self.type_of_path_expr.get(&expr).cloned(),
        }
    }
}
