use db::WorkspaceDataBase;

use crate::{
    HirNodeInfo,
    hir_def::expressions::{
        expression::{BeginPathExpr, Expr, InitExpr, ParamAssign, PathExpr, VariableAccess},
        invocation::Invocation,
        spec::Spec,
    },
    hir_ty::{
        head::{init_inference::infer_initialization, signature::infer_signature},
        ty::Type,
    },
};

pub mod cast;
pub mod coerce;
pub mod const_eval;
pub mod expr;
pub mod literals;
pub mod normalize;
pub mod table;

pub trait Infer<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db>;
}

impl<'db> Infer<'db> for Spec<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        infer_signature(db, self.get_scope_id(db))
            .type_of_specs
            .get(self)
            .copied()
            .unwrap_or_default()
    }
}

impl<'db> Infer<'db> for InitExpr<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        infer_initialization(db, self.get_scope_id(db))
            .init_expr_result
            .type_of_init_expr
            .get(self)
            .copied()
            .unwrap_or_default()
    }
}

// Each node below is recorded by the scope's statements or by its
// initializers, and answers alike from either (`ScopeId::inference`).

impl<'db> Infer<'db> for Invocation<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        self.get_scope_id(db)
            .inference(db)
            .type_of_invocation(*self)
    }
}

impl<'db> Infer<'db> for ParamAssign<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        match self
            .get_scope_id(db)
            .inference(db)
            .variable_for_param(*self)
        {
            Some(var) => Type::new_var(db, var),
            None => Type::Never,
        }
    }
}

impl<'db> Infer<'db> for VariableAccess<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        self.get_scope_id(db)
            .inference(db)
            .type_of_variable_access(*self)
    }
}

impl<'db> Infer<'db> for BeginPathExpr<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        self.get_scope_id(db)
            .inference(db)
            .type_of_begin_path_expr(*self)
    }
}

impl<'db> Infer<'db> for PathExpr<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        self.get_scope_id(db).inference(db).type_of_path_expr(*self)
    }
}

impl<'db> PathExpr<'db> {
    /// The array this bracket indexes and the dimensions of it consumed
    /// ([`IndexedArray`]), as the walk recorded them.
    ///
    /// [`IndexedArray`]: crate::hir_ty::body::IndexedArray
    pub fn indexed_array(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<crate::hir_ty::body::IndexedArray<'db>> {
        self.get_scope_id(db).inference(db).indexed_array(*self)
    }
}

impl<'db> Infer<'db> for Expr<'db> {
    fn infer(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        self.get_scope_id(db).inference(db).type_of_expr(*self)
    }
}

impl<'db> Expr<'db> {
    /// The expression's type WITH its adjustments applied.
    ///
    /// Indexing, field access and dereference are recorded as adjustments over
    /// a base type, so the plain [`Infer::infer`] type of `arr[0]` is the ARRAY,
    /// not its element. Consumers that want the type an expression actually
    /// *evaluates to* — codegen picking a machine type, overload resolution
    /// classifying an argument — must use this, or they have to reconstruct the
    /// adjustment themselves.
    pub fn infer_adjusted(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        self.get_scope_id(db)
            .inference(db)
            .type_of_expr_adjusted(*self)
    }
}
