use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    HirNodeInfo,
    check::errors::{
        analysis_error::ToIdeDiagnostic,
        body_inference::{BodyInferenceError, TypeError},
    },
    hir_def::{
        expressions::{
            expression::{Expr, PathExpr},
            spec::ElementarySpec,
        },
        scope::ScopeId,
    },
    hir_ty::{
        body_inference::BodyInferenceResult,
        resolver::Resolver,
        ty::{InferType, Type},
    },
};

pub struct CoerceError<'db> {
    pub expected: Type<'db>,
    pub actual: Type<'db>,
}

pub type CoerceResult<'db> = Result<(), CoerceError<'db>>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum PathOrExpr<'db> {
    Path(PathExpr<'db>),
    Expr(Expr<'db>),
}

#[derive(Default, Clone)]
pub struct InferenceTable<'db> {
    /// Current inference mode
    pub current_mode: InferMode<'db>,
    /// Map of paths or expressions to their *non* inferred types
    pub types: FxHashMap<Expr<'db>, Type<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferMode<'db> {
    // No inference needed
    NoInfer,
    // Still unresolved
    Unresolved,
    // Resolved to an infer type
    ResolvedInfer { ty: Type<'db>, expr: Expr<'db> },
    // Fully resolved type
    Resolved { ty: Type<'db>, expr: Expr<'db> },
}

impl Default for InferMode<'_> {
    fn default() -> Self {
        InferMode::NoInfer
    }
}

impl<'db> InferenceTable<'db> {
    pub fn new() -> Self {
        Self {
            current_mode: InferMode::NoInfer,
            types: FxHashMap::default(),
        }
    }

    /// Forces the current inference to a resolved type
    pub fn set_resolved(&mut self, expr: Expr<'db>, ty: Type<'db>) {
        self.current_mode = InferMode::Resolved { ty, expr };
    }

    /// Adds a new type to the inference table
    ///
    /// depending on the current inference mode, it may update the mode
    pub fn add_type(
        &mut self,
        db: &'db dyn BaseDatabase,
        expr: Expr<'db>,
        value: Type<'db>,
        resolver: Resolver<'db>,
    ) {
        match value.shallow(db) {
            Type::Infer(infer) => {
                match self.current_mode {
                    // sets the inferred type based on the infer type
                    InferMode::NoInfer | InferMode::Unresolved => {
                        self.current_mode = InferMode::ResolvedInfer {
                            ty: match infer {
                                InferType::Integer(int) => Type::Elementary(ElementarySpec::LInt),
                                InferType::Float(flt) => Type::Elementary(ElementarySpec::LReal),
                            },
                            expr,
                        };
                    }
                    InferMode::Resolved { .. } | InferMode::ResolvedInfer { .. } => {}
                }
                self.types.insert(expr, value);
            }
            Type::Elementary(elem) => match self.current_mode {
                InferMode::NoInfer | InferMode::Unresolved | InferMode::ResolvedInfer { .. } => {
                    self.current_mode = InferMode::Resolved { ty: value, expr };
                }
                InferMode::Resolved { .. } => {}
            },
            _ => { /* ignore */ }
        }
    }

    /// Resolves completely all inferred types in the table
    pub fn resolve_completly(
        &self,
        db: &'db dyn BaseDatabase,
        resolver: Resolver<'db>,
        results: &mut BodyInferenceResult<'db>,
    ) {
        // Determine the final concrete type we substitute with
        let (final_ty, source) = match self.current_mode {
            InferMode::Resolved { ty, expr } => (ty, expr),
            InferMode::ResolvedInfer { ty, expr } => (ty, expr),
            _ => {
                // no inference to resolve
                // therefore all types are left as is
                return;
            }
        };

        // Insert for each expression the final resolved type
        for (expr, ty) in &self.types {
            match ty {
                Type::Infer(infer) => {
                    if let Err(err) = final_ty.coerce_with(
                        db,
                        match infer {
                            InferType::Integer(int) => Type::Elementary(ElementarySpec::LInt),
                            InferType::Float(flt) => Type::Elementary(ElementarySpec::LReal),
                        },
                        resolver,
                    ) {
                        results.errors.push(
                            BodyInferenceError::TypeMismatch(TypeError::CannotInfer {
                                source: source,
                                target: final_ty,
                                value: *ty,
                                expr: *expr,
                            })
                            .to_diagnostic(db),
                        );
                    }
                    results.type_of_expr.insert(*expr, final_ty);
                }
                _ => (),
            };
        }
    }
}

impl<'db> Type<'db> {
    // Type coercion check
    #[must_use]
    pub fn coerce_with(
        &self,
        db: &'db dyn BaseDatabase,
        to: Type<'db>,
        resolver: Resolver<'db>,
    ) -> CoerceResult<'db> {
        // We return true if the lhs or rhs is of type never.
        // that's because Never variants are already reported by the resolver and we don't want to propagate too many errors

        if self.is_never() || to.is_never() {
            return Ok(());
        }

        // shallowing here is necessary here to avoid matching on wrapped types
        let lhs = self.shallow(db);
        let to = to.shallow(db);

        match (lhs, &to) {
            (typ1, Type::DataType(typ)) => {
                typ1.coerce_with(db, Type::new_spec(db, typ.spec(db)), resolver)
            }
            (var1, Type::Variable(var)) => {
                var1.coerce_with(db, Type::new_spec(db, var.spec(db)), resolver)
            }
            // variant is already solved by the resolver
            (Type::Enum(e1), Type::EnumVariant(e2)) => Ok(()),
            // same types are assignable
            (Type::Struct(s1), Type::Struct(s2)) => {
                return match s1.eq(s2) {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: to,
                    }),
                };
            }
            // check element spec equality
            (Type::StructElement(elem), rhs) => {
                Type::new_spec(db, elem.spec(db)).coerce_with(db, *rhs, resolver)
            }
            // same types are assignable
            (Type::Array(a1), Type::Array(a2)) => {
                return match a1.eq(a2) {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: to,
                    }),
                };
            }
            // check array spec equality
            (Type::Array(a1), rhs) => {
                Type::new_spec(db, a1.of_type(db)).coerce_with(db, *rhs, resolver)
            }
            // check subrange base type equality
            (Type::SubRange(sub), rhs) => {
                Type::new_spec(db, sub._type(db)).coerce_with(db, *rhs, resolver)
            }
            (Type::Elementary(lhs), Type::Elementary(rhs)) => {
                if lhs == *rhs {
                    return Ok(());
                }
                // try implicit conversions in both directions
                match !lhs.implicit_cast(*rhs).is_never() || !rhs.implicit_cast(lhs).is_never() {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: to,
                    }),
                }
            }
            (Type::Elementary(lhs), Type::Function(f)) => {
                if let Some(ret_ty) = f.return_type(db) {
                    self.coerce_with(db, Type::new_spec(db, *ret_ty), resolver)
                } else {
                    Err(CoerceError {
                        expected: *self,
                        actual: Type::Void,
                    })
                }
            }
            (Type::Elementary(lhs), Type::MethodDecl(f)) => {
                if let Some(ret_ty) = f.return_type(db) {
                    self.coerce_with(db, Type::new_spec(db, *ret_ty), resolver)
                } else {
                    Err(CoerceError {
                        expected: *self,
                        actual: Type::Void,
                    })
                }
            }
            // Assignments to function /method are allowed *only inside their body*
            (Type::Function(f), rhs) => {
                if let Some(ret_ty) = f.return_type(db) {
                    Type::new_spec(db, *ret_ty).coerce_with(db, *rhs, resolver)
                } else {
                    Err(CoerceError {
                        expected: *self,
                        actual: Type::Void,
                    })
                }
            }
            (Type::MethodDecl(m), rhs) => {
                if let Some(ret_ty) = m.return_type(db) {
                    Type::new_spec(db, *ret_ty).coerce_with(db, *rhs, resolver)
                } else {
                    Err(CoerceError {
                        expected: *self,
                        actual: Type::Void,
                    })
                }
            }
            (Type::RefTo(_), Type::Null) => Ok(()),
            _ => Err(CoerceError {
                expected: *self,
                actual: to,
            }),
        }
    }
}
