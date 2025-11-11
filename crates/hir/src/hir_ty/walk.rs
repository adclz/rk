use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, Related};

use crate::{
    AstId, HirNodeInfo, TypeInfo, check::errors::path_error::AccessError, hir_def::{
        expressions::{
            expression::PathExpr,
            spec::{Spec, SpecKind, StructElement},
        }, pous::{
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        }, scope::{ScopeId, ScopeKind}, semantic_index::get_scope, visibility::Visibility
    }, hir_ty::{
        flatten::PathExprWalkStep,
        inheritance_solver::{InheritedMethodSet, MethodRef, inherited_methods},
        name_res::resolve_namespace_access,
        ty::{Ty, TyKind},
        ty_var_access_resolver::CallSite,
    }
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
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        // We do not use the id field of the expression because of the way PathExpr are built from the AST.
        // Instead, get_id method will return the id of *this* specific path element.
        self.expr.get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.expr.scope_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedPathResult<'db> {
    Ok(ResolvedPath<'db>),
    Err(AccessError<'db>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Adjustement<'db> {
    None,
    Deref(Box<ResolvedPath<'db>>),
    Array(Box<ResolvedPath<'db>>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResolvedPath<'db> {
    pub expr: CallSite<'db>,
    pub adjustement: Adjustement<'db>,
    pub kind: ResolvedPathKind<'db>,
}

impl<'db> HirNodeInfo<'db> for ResolvedPath<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.expr.get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.expr.get_scope_id(db)
    }
}

impl<'db> ResolvedPath<'db> {
    pub fn as_var(&self, db: &'db dyn BaseDatabase) -> Option<VariableDecl<'db>> {
        match self.kind {
            ResolvedPathKind::Variable(v) => Some(v),
            _ => None,
        }
    }

    pub fn target_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        match &self.kind {
            ResolvedPathKind::Pou(pou)
            | ResolvedPathKind::This(pou)
            | ResolvedPathKind::Super(pou)
            | ResolvedPathKind::SuperBody(pou) => pou.get_scope_id(db),
            ResolvedPathKind::Variable(v) => v.get_scope_id(db),
            ResolvedPathKind::StructElement(e) => e.get_scope_id(db),
            ResolvedPathKind::Spec(s) => s.get_scope_id(db),
            ResolvedPathKind::Method(m) => m.get_scope_id(db),
        }
    }

    pub fn is_callable(&self, db: &'db dyn BaseDatabase) -> bool {
        match &self.kind {
            ResolvedPathKind::Pou(pou) => match pou.pou(db) {
                Pou::Function(_) => true,
                _ => false,
            },
            ResolvedPathKind::Variable(v) => match v.spec(db).to_ty(db).kind(db) {
                TyKind::Function(_) | TyKind::FunctionBlock(_) => true,
                _ => false,
            },
            ResolvedPathKind::Method(_) => true,
            _ => false,
        }
    }

    pub fn visibility(&self, db: &'db dyn BaseDatabase) -> Visibility {
        match self.kind {
            ResolvedPathKind::Method(m) => m.visibility(db),
            // Todo: add variables
            _ => Visibility::PUBLIC
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedPathKind<'db> {
    This(PouDecl<'db>),
    SuperBody(PouDecl<'db>),
    Super(PouDecl<'db>),
    Pou(PouDecl<'db>),
    Variable(VariableDecl<'db>),
    StructElement(StructElement<'db>),
    Spec(Spec<'db>),
    Method(MethodRef<'db>),
}

impl<'db> ResolvedPathKind<'db> {
    pub fn with_call_site(
        self,
        db: &'db dyn BaseDatabase,
        expr: PathExpr<'db>,
        adjustement: Adjustement<'db>,
    ) -> ResolvedPath<'db> {
        ResolvedPath {
            kind: self,
            expr: CallSite::new(expr.scope_id(db), expr.id(db)),
            adjustement,
        }
    }

    pub fn decl_name(&self, db: &'db dyn BaseDatabase) -> String {
        match &self {
            ResolvedPathKind::Variable(v) => v.name(db).text(db).to_string(),
            ResolvedPathKind::Pou(p)
            | ResolvedPathKind::Super(p)
            | ResolvedPathKind::SuperBody(p)
            | ResolvedPathKind::This(p) => p.name(db).text(db).to_string(),
            ResolvedPathKind::Method(m) => m.name(db).text(db).to_string(),
            ResolvedPathKind::Spec(m) => m.to_ty(db).type_name(db),
            ResolvedPathKind::StructElement(m) => m.name(db).text(db).to_string(),
        }
    }

    pub fn diag_with_location(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        let (file, span, decl_name, name) = match &self {
            ResolvedPathKind::Pou(p)
            | ResolvedPathKind::This(p)
            | ResolvedPathKind::Super(p)
            | ResolvedPathKind::SuperBody(p) => (
                p.get_scope_id(db).file(db),
                p.name_span(db),
                "pou",
                p.name(db).text(db).to_string(),
            ),
            ResolvedPathKind::Variable(v) => (
                v.get_scope_id(db).file(db),
                v.name_span(db),
                "variable",
                v.name(db).text(db).to_string(),
            ),
            ResolvedPathKind::StructElement(f) => (
                f.get_scope_id(db).file(db),
                f.name_span(db),
                "struct element",
                f.name(db).text(db).to_string(),
            ),
            ResolvedPathKind::Method(m) => (
                m.get_scope_id(db).file(db),
                m.name_span(db),
                "method",
                m.name(db).text(db).to_string(),
            ),
            ResolvedPathKind::Spec(s) => (
                s.get_scope_id(db).file(db),
                s.get_span(db),
                "",
                s.to_ty(db).type_name(db),
            ),
        };

        diag.with_related(Related::new(
            format!("{decl_name} '{name}' declared here"),
            file,
            span,
        ));
    }
}

impl<'db> HirNodeInfo<'db> for ResolvedPathKind<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            ResolvedPathKind::Pou(p)
            | ResolvedPathKind::Super(p)
            | ResolvedPathKind::SuperBody(p)
            | ResolvedPathKind::This(p) => p.get_id(db),
            ResolvedPathKind::Variable(v) => v.get_id(db),
            ResolvedPathKind::StructElement(e) => e.get_id(db),
            ResolvedPathKind::Spec(s) => s.get_id(db),
            ResolvedPathKind::Method(m) => m.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        match self {
            ResolvedPathKind::Pou(p)
            | ResolvedPathKind::Super(p)
            | ResolvedPathKind::SuperBody(p)
            | ResolvedPathKind::This(p) => p.get_scope_id(db),
            ResolvedPathKind::Variable(v) => v.get_scope_id(db),
            ResolvedPathKind::StructElement(e) => e.get_scope_id(db),
            ResolvedPathKind::Spec(s) => s.get_scope_id(db),
            ResolvedPathKind::Method(m) => m.get_scope_id(db),
        }
    }
}

pub trait ResolvePath<'db> {
    fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        step: PathExprWalkStep<'db>
    ) -> Option<ResolvedPath>;
}

impl<'db> ResolvedPath<'db> {
    /// Tries to resolve a [`ResolvedPath`] to a [`Ty`] enum.
    ///
    /// If [`TyKind`] variant is Err, this function will return an [`AccessError`]
    pub fn try_to_ty(&self, db: &'db dyn BaseDatabase) -> Result<Ty<'db>, AccessError<'db>> {
        match self.adjustement {
            Adjustement::Array(ref arr) => arr.try_to_ty(db),
            Adjustement::Deref(ref deref) => deref.try_to_ty(db),
            _ => {
                let result = match &self.kind {
                    ResolvedPathKind::Variable(v) => v.spec(db).to_ty(db),
                    ResolvedPathKind::StructElement(e) => e.spec(db).to_ty(db),
                    ResolvedPathKind::Spec(s) => s.to_ty(db),
                    ResolvedPathKind::Method(m) => m.
                        return_type(db)
                        .map(|spec| spec.to_ty(db))
                        .ok_or_else(|| AccessError::InvalidTypeAccess {
                                access: self.clone(),
                            })?,
                    ResolvedPathKind::Pou(pou) => match pou.pou(db) {
                        Pou::DataType(dt) => dt.spec(db).to_ty(db),
                        // Special case for functions to get their return type
                        Pou::Function(f) => f
                            .return_type(db)
                            .map(|spec| spec.to_ty(db))
                            .ok_or_else(|| AccessError::InvalidTypeAccess {
                                access: self.clone(),
                            })?,
                        _ => {
                            return Err(AccessError::InvalidTypeAccess {
                                access: self.clone(),
                            });
                        }
                    },
                    _ => {
                        return Err(AccessError::InvalidTypeAccess {
                            access: self.clone(),
                        })?;
                    }
                };
                match result.kind(db) {
                    TyKind::Err(e) => Err(e.clone()),
                    any => Ok(result),
                }
            }
        }
    }

    pub fn diag_with_location(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        self.kind.diag_with_location(db, diag);
    }

    pub fn decl_name(&self, db: &'db dyn BaseDatabase) -> String {
        self.kind.decl_name(db)
    }

    pub fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        step: &'db PathExprWalkStep<'db>,
    ) -> Result<ResolvedPath<'db>, AccessError<'db>> {
        match self.adjustement {
            Adjustement::Deref(ref deref) => return deref.walk(db, step),
            Adjustement::Array(ref array) => return array.walk(db, step),
            _ => {}
        }
        match &self.kind {
            ResolvedPathKind::Pou(pou) | ResolvedPathKind::SuperBody(pou) => pou.walk(db, step),
            ResolvedPathKind::This(pou) => match step {
                PathExprWalkStep::Field { ident, expr } => {
                    if let Some(m) = pou.scope_id(db).def_map(db).declared_methods.get(&ident.ident) {
                        return Ok(ResolvedPathKind::Method(*m).with_call_site(
                            db,
                            *expr,
                            Adjustement::None,
                        ));
                    }

                    Err(AccessError::UnknownMethod {
                        ty: self.clone(),
                        expr: *expr,
                    })
                }
                _ => Err(AccessError::InvalidTypeAccess {
                    access: self.clone(),
                })?,
            },
            ResolvedPathKind::Super(p) => inherited_methods(db, *p).walk(db, self, step),
            ResolvedPathKind::Variable(var) => {
                var.spec(db)
                    .walk(db, ResolvedPathKind::Variable(*var), step)
            }
            ResolvedPathKind::Spec(spec) => spec.walk(db, ResolvedPathKind::Spec(*spec), step),
            ResolvedPathKind::StructElement(field) => {
                field
                    .spec(db)
                    .walk(db, ResolvedPathKind::StructElement(*field), step)
            }
            ResolvedPathKind::Method(m) => Err(AccessError::InvalidTypeAccess {
                access: self.clone(),
            })?,
        }
    }
}

impl<'db> InheritedMethodSet<'db> {
    pub fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        parent: &ResolvedPath<'db>,
        step: &'db PathExprWalkStep<'db>,
    ) -> Result<ResolvedPath<'db>, AccessError<'db>> {
        match step {
            PathExprWalkStep::Field { ident, expr } => {
                if let Some(m) = self.methods.get(&ident.ident) {
                    return Ok(ResolvedPathKind::Method(m.method).with_call_site(
                        db,
                        *expr,
                        Adjustement::None,
                    ));
                }

                Err(AccessError::UnknownMethod {
                    ty: parent.clone(),
                    expr: *expr,
                })
            }
            _ => Err(AccessError::InvalidTypeAccess {
                access: parent.clone(),
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
                match self.pou(db) {
                    Pou::Class(_) | Pou::Function(_) | Pou::FunctionBlock(_) => {
                        if let Some(var) = self.scope_id(db).def_map(db).global_variables.get(&ident.ident)
                        {
                            return Ok(ResolvedPathKind::Variable(*var).with_call_site(
                                db,
                                *expr,
                                Adjustement::None,
                            ));
                        } else if let Some(m) = self.scope_id(db).def_map(db).declared_methods.get(&ident.ident) {
                            return Ok(ResolvedPathKind::Method(*m).with_call_site(
                                db,
                                *expr,
                                Adjustement::None,
                            ));
                        }
                    }
                    Pou::DataType(dt) => {
                        return dt.spec(db).walk(db, ResolvedPathKind::Pou(*self), step);
                    }
                    _ => {}
                }

                // Check if ident == self name
                if *ident == *self.name(db) {
                    return Ok(ResolvedPathKind::Pou(*self).with_call_site(
                        db,
                        *expr,
                        Adjustement::None,
                    ));
                }

                Err(AccessError::UnknownField {
                    ty: ResolvedPathKind::Pou(*self).with_call_site(db, *expr, Adjustement::None),
                    expr: *expr,
                })
            }
            // Deref case, same as Field except we need to check if the target is a Reference
            PathExprWalkStep::Deref { target, expr } => {
                match self.scope_id(db).def_map(db).global_variables.get(&target.ident) {
                    Some(var) => match var.spec(db).kind(db) {
                        SpecKind::Ref(ref_to) => Ok(ResolvedPathKind::Variable(*var)
                            .with_call_site(
                                db,
                                *expr,
                                Adjustement::Deref(Box::new(
                                    ResolvedPathKind::Spec(*ref_to).with_call_site(
                                        db,
                                        *expr,
                                        Adjustement::None,
                                    ),
                                )),
                            )),
                        _ => Err(AccessError::NotAReference {
                            ty: ResolvedPathKind::Variable(*var).with_call_site(
                                db,
                                *expr,
                                Adjustement::None,
                            ),
                            expr: *step.get_expr(),
                        }),
                    },
                    _ => Err(AccessError::NoLocalItemInScope {
                        expr: *step.get_expr(),
                    }),
                }
            }
            // Index case is invalid
            PathExprWalkStep::Index { expr } => match self.pou(db) {
                Pou::DataType(dt) => dt.spec(db).walk(db, ResolvedPathKind::Pou(*self), step),
                _ => Err(AccessError::NotAnArray {
                    ty: ResolvedPathKind::Pou(*self).with_call_site(db, *expr, Adjustement::None),
                    expr: *expr,
                }),
            },
        }
    }
}

impl<'db> MethodRef<'db> {
    pub fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        step: &'db PathExprWalkStep<'db>,
    ) -> Result<ResolvedPath<'db>, AccessError<'db>> {
        match step {
            // Simplest case, just an identifier
            PathExprWalkStep::Field { ident, expr } => {
                if let Some(var) = self.get_scope_id(db).def_map(db).global_variables.get(&ident.ident) {
                    return Ok(ResolvedPathKind::Variable(*var).with_call_site(
                        db,
                        *expr,
                        Adjustement::None,
                    ));
                }

                let parent =  get_scope(db, self.get_scope_id(db)).parent.expect("Methods always have a paent scope");

                let parent_scope = get_scope(db, parent);
                match parent_scope.kind {
                    ScopeKind::Pou(p) => {
                        return p.walk(db, step)
                    },
                    _ => {}
                }
                Err(AccessError::UnknownField {
                    ty: ResolvedPathKind::Method(*self).with_call_site(
                        db,
                        *expr,
                        Adjustement::None,
                    ),
                    expr: *expr,
                })
            }
            // Deref case, same as Field except we need to check if the target is a Reference
            PathExprWalkStep::Deref { target, expr } => {
                match self
                    .get_scope_id(db)
                    .def_map(db)
                    .global_variables
                    .get(&target.ident)
                {
                    Some(var) => match var.spec(db).kind(db) {
                        SpecKind::Ref(ref_to) => Ok(ResolvedPathKind::Variable(*var)
                            .with_call_site(
                                db,
                                *expr,
                                Adjustement::Deref(Box::new(
                                    ResolvedPathKind::Spec(*ref_to).with_call_site(
                                        db,
                                        *expr,
                                        Adjustement::None,
                                    ),
                                )),
                            )),
                        _ => Err(AccessError::NotAReference {
                            ty: ResolvedPathKind::Variable(*var).with_call_site(
                                db,
                                *expr,
                                Adjustement::None,
                            ),
                            expr: *step.get_expr(),
                        }),
                    },
                    _ => Err(AccessError::NoLocalItemInScope {
                        expr: *step.get_expr(),
                    }),
                }
            }
            // Index case is invalid
            PathExprWalkStep::Index { expr } => Err(AccessError::NotAnArray {
                ty: ResolvedPathKind::Method(*self).with_call_site(db, *expr, Adjustement::None),
                expr: *expr,
            }),
        }
    }
}

impl<'db> Spec<'db> {
    pub fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        origin: ResolvedPathKind<'db>,
        step: &'db PathExprWalkStep<'db>,
    ) -> Result<ResolvedPath<'db>, AccessError<'db>> {
        // resolve the target if it is one
        if let SpecKind::Target(target) = self.kind(db) {
            return match resolve_namespace_access(db, &target.path) {
                Some(pou) => pou.walk(db, step),
                None => Err(AccessError::NoLocalItemInScope {
                    expr: *step.get_expr(),
                }),
            };
        }

        match step {
            // Only a struct can have fields
            PathExprWalkStep::Field { ident, expr } => match self.kind(db) {
                SpecKind::Struct(strukt) => match strukt.resolve_elements(db).get(&ident.ident) {
                    Some(field) => Ok(ResolvedPathKind::StructElement(*field).with_call_site(
                        db,
                        *expr,
                        Adjustement::None,
                    )),
                    None => Err(AccessError::UnknownField {
                        ty: ResolvedPathKind::Spec(*self).with_call_site(
                            db,
                            *expr,
                            Adjustement::None,
                        ),
                        expr: *expr,
                    }),
                },
                _ => Err(AccessError::TypeHasNoField {
                    ty: ResolvedPathKind::Spec(*self).with_call_site(db, *expr, Adjustement::None),
                    expr: *expr,
                }),
            },
            // Same, but we need to deref first
            PathExprWalkStep::Deref { target, expr } => match self.kind(db) {
                SpecKind::Struct(strukt) => match strukt.resolve_elements(db).get(&target.ident) {
                    Some(field) => match field.spec(db).kind(db) {
                        SpecKind::Ref(ref_to) => Ok(ResolvedPathKind::StructElement(*field)
                            .with_call_site(
                                db,
                                *expr,
                                Adjustement::Deref(Box::new(
                                    ResolvedPathKind::Spec(*ref_to).with_call_site(
                                        db,
                                        *expr,
                                        Adjustement::None,
                                    ),
                                )),
                            )),
                        _ => Err(AccessError::NotAReference {
                            ty: ResolvedPathKind::StructElement(*field).with_call_site(
                                db,
                                *expr,
                                Adjustement::None,
                            ),
                            expr: *step.get_expr(),
                        }),
                    },
                    None => Err(AccessError::UnknownField {
                        ty: ResolvedPathKind::Spec(*self).with_call_site(
                            db,
                            *expr,
                            Adjustement::None,
                        ),
                        expr: *step.get_expr(),
                    }),
                },
                _ => Err(AccessError::TypeHasNoField {
                    ty: ResolvedPathKind::Spec(*self).with_call_site(db, *expr, Adjustement::None),
                    expr: *step.get_expr(),
                }),
            },
            PathExprWalkStep::Index { expr } => match self.kind(db) {
                SpecKind::Array(array) => Ok(origin.with_call_site(
                    db,
                    *expr,
                    Adjustement::Array(Box::new(
                        ResolvedPathKind::Spec(array.of_type(db)).with_call_site(
                            db,
                            *expr,
                            Adjustement::None,
                        ),
                    )),
                )),
                _ => Err(AccessError::NotAnArray {
                    ty: ResolvedPathKind::Spec(*self).with_call_site(db, *expr, Adjustement::None),
                    expr: *step.get_expr(),
                }),
            },
        }
    }
}
