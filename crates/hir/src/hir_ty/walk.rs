use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::path_error::AccessError, hir_def::{
        expressions::{
            expression::PathExpr,
            spec::{Spec, SpecKind, StructElement},
        },
        pous::{
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        scope::FileScopeId,
    }, hir_ty::{
        inheritance_solver::MethodRef, name_res::resolve_namespace_access, signatures::LocalVariables, ty::Ty, ty_var_access_resolver::{resolve_var_access, PathExprWalkStep}
    }, AstId, HirNodeInfo, TypeInfo
};

/// Represents a resolved element in a path expression.
/// Contains the expression and its type.
#[derive(Debug, Clone, PartialEq, Hash, Eq, salsa::Update)]
pub struct ResolvedPathElement<'db> {
    // The expression that was resolved
    pub expr: PathExpr<'db>,
    // The type of the resolved element
    pub kind: ResolvedPathResult<'db>,
}

impl<'db> ResolvedPathElement<'db> {
    pub fn new(expr: PathExpr<'db>, kind: ResolvedPathResult<'db>) -> Self {
        Self { expr, kind }
    }

    pub fn get_expr(&self) -> &PathExpr<'db> {
        &self.expr
    }
}

impl<'db> HirNodeInfo<'db> for ResolvedPathElement<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        // We do not use the id field of the expression because of the way PathExpr are built from the AST.
        // Instead, get_id method will return the id of *this* specific path element.
        self.expr.get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.expr.scope_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedPathResult<'db> {
    Ok(ResolvedPath<'db>),
    Err(AccessError<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedPath<'db> {
    Pou(PouDecl<'db>),
    Variable(VariableDecl<'db>),
    StructElement(StructElement<'db>),
    Spec(Spec<'db>),
    Method(MethodRef<'db>),
    Deref {
        origin: Box<ResolvedPath<'db>>,
        element: Box<ResolvedPath<'db>>,
    },
    Array {
        origin: Box<ResolvedPath<'db>>,
        element: Box<ResolvedPath<'db>>,
    },
}

impl<'db> ResolvedPath<'db> {
    pub fn try_to_try(&self, db: &'db dyn BaseDatabase) -> Result<Ty<'db>, AccessError<'db>> {
        Ok(match self {
            ResolvedPath::Pou(p) => match p.pou(db) {
                Pou::Function(f) => f
                    .return_type(db)
                    .ok_or_else(|| AccessError::InvalidTypeAccess {
                        access: self.clone(),
                    })?
                    .to_ty(db),
                _ => {
                    return Err(AccessError::InvalidTypeAccess {
                        access: self.clone(),
                    })?;
                }
            },
            ResolvedPath::Method(m) => m
                .return_type(db)
                .ok_or_else(|| AccessError::InvalidTypeAccess {
                    access: self.clone(),
                })?
                .to_ty(db),
            ResolvedPath::Variable(v) => v.spec(db).to_ty(db),
            ResolvedPath::StructElement(e) => e.spec(db).to_ty(db),
            ResolvedPath::Spec(s) => s.to_ty(db),
            ResolvedPath::Deref { element, .. } => element.try_to_try(db)?,
            ResolvedPath::Array { element, .. } => element.try_to_try(db)?,
            _ => Err(AccessError::InvalidTypeAccess {
                access: self.clone(),
            })?,
        })
    }
}

impl<'db> HirNodeInfo<'db> for ResolvedPath<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            ResolvedPath::Pou(p) => p.get_id(db),
            ResolvedPath::Variable(v) => v.get_id(db),
            ResolvedPath::StructElement(f) => f.get_id(db),
            ResolvedPath::Spec(s) => s.get_id(db),
            ResolvedPath::Method(m) => m.get_id(db),
            ResolvedPath::Deref { origin, .. } => origin.get_id(db),
            ResolvedPath::Array { origin, .. } => origin.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self {
            ResolvedPath::Pou(p) => p.get_scope_id(db),
            ResolvedPath::Variable(v) => v.get_scope_id(db),
            ResolvedPath::StructElement(f) => f.get_scope_id(db),
            ResolvedPath::Spec(s) => s.get_scope_id(db),
            ResolvedPath::Method(m) => m.get_scope_id(db),
            ResolvedPath::Deref { origin, .. } => origin.get_scope_id(db),
            ResolvedPath::Array { origin, .. } => origin.get_scope_id(db),
        }
    }
}

impl<'db> ResolvedPath<'db> {
    pub fn decl_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            ResolvedPath::Variable(v) => v.name(db).text(db).to_string(),
            ResolvedPath::Pou(p) => p.name(db).text(db).to_string(),
            ResolvedPath::Method(m) => m.name(db).text(db).to_string(),
            ResolvedPath::Spec(m) => m.to_ty(db).type_name(db),
            ResolvedPath::StructElement(m) => m.name(db).text(db).to_string(),
            ResolvedPath::Deref { origin, element } => {
                format!("{}.{}", origin.decl_name(db), element.decl_name(db))
            }
            ResolvedPath::Array { origin, element } => {
                format!("{}[{}]", origin.decl_name(db), element.decl_name(db))
            }
        }
    }
}

impl<'db> ResolvedPath<'db> {
    pub fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        step: &'db PathExprWalkStep<'db>,
    ) -> Result<ResolvedPath<'db>, AccessError<'db>> {
        match self {
            ResolvedPath::Pou(pou) => pou.walk(db, step),
            ResolvedPath::Variable(var) => var.spec(db).walk(db, step),
            ResolvedPath::Spec(spec) => spec.walk(db, step),
            ResolvedPath::StructElement(field) => field.spec(db).walk(db, step),
            ResolvedPath::Deref { origin, element } => element.walk(db, step),
            ResolvedPath::Array { origin, element } => element.walk(db, step),
            ResolvedPath::Method(m) => Err(AccessError::InvalidTypeAccess {
                access: self.clone(),
            })?,
        }
    }
}

impl<'db> PouDecl<'db> {
    pub fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        step: &'db PathExprWalkStep<'db>,
    ) -> Result<ResolvedPath<'db>, AccessError<'db>> {
        match step {
            // Simplest case, just an identifier
            PathExprWalkStep::Field { ident, expr } => {
                // Check local variables of pou
                if let Some(var) = self.local_variables(db).get(&ident.ident) {
                    return Ok(ResolvedPath::Variable(*var));
                }

                // Check if ident == self name
                if *ident == *self.name(db) {
                    return Ok(ResolvedPath::Pou(*self));
                }

                Err(AccessError::UnknownField {
                    ty: ResolvedPath::Pou(*self),
                    expr: *step.get_expr(),
                })
            }
            // Deref case, same as Field except we need to check if the target is a Reference
            PathExprWalkStep::Deref { target, expr } => {
                match self.local_variables(db).get(&target.ident) {
                    Some(var) => match var.spec(db).kind(db) {
                        SpecKind::Ref(ref_to) => return Ok(ResolvedPath::Variable(*var)),
                        _ => {
                            return Err(AccessError::NotAReference {
                                ty: ResolvedPath::Variable(*var),
                                expr: *step.get_expr(),
                            });
                        }
                    },
                    _ => Err(AccessError::NoItemInScope {
                        expr: *step.get_expr(),
                    }),
                }
            }
            // Index case is invalid
            PathExprWalkStep::Index { expr } => Err(AccessError::NotAnArray {
                ty: ResolvedPath::Pou(*self),
                expr: *step.get_expr(),
            })?,
        }
    }
}

impl<'db> Spec<'db> {
    pub fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        step: &'db PathExprWalkStep<'db>,
    ) -> Result<ResolvedPath<'db>, AccessError<'db>> {
        match step {
            // Only a struct can have fields
            PathExprWalkStep::Field { ident, expr } => match self.kind(db) {
                SpecKind::Target(t) => match resolve_namespace_access(
                    db,
                    self.get_scope_id(db),
                    t.path
                ) {
                    Some(pou) => pou.walk(db, step),
                    None => Err(AccessError::NoItemInScope { expr: *step.get_expr()}),
                },
                SpecKind::Struct(strukt) => match strukt.resolve_elements(db).get(&ident.ident) {
                    Some(field) => return Ok(ResolvedPath::StructElement(*field)),
                    None => return Err(AccessError::UnknownField { ty: ResolvedPath::Spec(*self), expr: *step.get_expr() }),
                },
                _ => Err(AccessError::TypeHasNoField { ty: ResolvedPath::Spec(*self), expr: *step.get_expr() })
            },
            // Same, but we need to deref first
            PathExprWalkStep::Deref { target, expr } => match self.kind(db) {
                SpecKind::Target(t) => match resolve_namespace_access(
                    db,
                    self.get_scope_id(db),
                    t.path
                ) {
                    Some(pou) => pou.walk(db, step),
                    None => Err(AccessError::NoItemInScope { expr: *step.get_expr()}),
                },
                SpecKind::Struct(strukt) => match strukt.resolve_elements(db).get(&target.ident) {
                    Some(field) => match field.spec(db).kind(db) {
                        SpecKind::Ref(ref_to) => return Ok(ResolvedPath::StructElement(*field)),
                        _ => return Err(AccessError::NotAReference { ty: ResolvedPath::StructElement(*field), expr: *step.get_expr() })
                    },
                    None => return Err(AccessError::UnknownField { ty: ResolvedPath::Spec(*self), expr: *step.get_expr() })
                },
                _ => {
                    return Err(AccessError::TypeHasNoField { ty: ResolvedPath::Spec(*self), expr: *step.get_expr() });
                }
            },
            PathExprWalkStep::Index { expr } => match self.kind(db) {
                SpecKind::Array(array) => {
                    return Ok(ResolvedPath::Array {
                        origin: Box::new(ResolvedPath::Spec(*self)),
                        element: Box::new(ResolvedPath::Spec(*array.of_type)),
                    });
                }
                _ => {
                    return Err(AccessError::NotAnArray {
                        ty: ResolvedPath::Spec(*self),
                        expr: *step.get_expr(),
                    });
                }
            },
        }
    }
}
