use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    CallSite,
    check::errors::{analysis_error::ToIdeDiagnostic, e3_type::TypeError},
    hir_def::expressions::expression::Expr,
    hir_ty::{
        body_inference::BodyInferenceResult,
        resolver::Resolver,
        ty::{InferType, Type},
    },
};

/*
    The inference table is used to resolve inferred types during body inference.

    It replaces all Type::Infer types with concrete types based on the context of their usage.
    If no concrete type can be determined, the inferred type is replaced with [`Type::Never`].

*/

#[derive(Default, Debug, Clone)]
pub struct InferenceTable<'db> {
    /// Current inference mode
    pub current_mode: InferMode<'db>,
    /// Map of paths or expressions to their *non* inferred types
    pub types: FxHashMap<Expr<'db>, Type<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InferMode<'db> {
    // No inference needed
    #[default]
    NoInfer,
    // Still unresolved
    Unresolved,
    // Resolved to an infer type
    ResolvedInfer {
        ty: Type<'db>,
        expr: Option<CallSite<'db>>,
    },
    // Fully resolved type
    // Resolved has a priority over ResolvedInfer
    Resolved {
        ty: Type<'db>,
        expr: Option<CallSite<'db>>,
    },
}

impl<'db> InferenceTable<'db> {
    pub fn new() -> Self {
        Self {
            current_mode: InferMode::NoInfer,
            types: FxHashMap::default(),
        }
    }

    /// Forces the current inference to a resolved type
    pub fn set_target_type(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        expr: Option<CallSite<'db>>,
        ty: Type<'db>,
    ) {
        // only elementary types can be used to resolve the inference
        // the inference table will only attempt to resolve infer variants, it does not check coercion
        let normalized = ty.normalize(db);
        let ty = match normalized {
            Type::SubRange(sub) => Type::new_spec(db, sub._type(db)),
            // we also accept bools ... because 0 and 1 literals can be boolean or numeric
            Type::Elementary(_) if normalized.is_numeric() || normalized.is_boolean() => ty,
            _ => return,
        };
        self.current_mode = InferMode::Resolved { ty, expr };
    }

    /// Adds a new type to the inference table
    ///
    /// depending on the current inference mode, it may update the mode
    pub fn add_type(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        expr: Expr<'db>,
        value: Type<'db>,
        resolver: Resolver<'db>,
    ) {
        let value = value.normalize(db);
        match value {
            Type::Infer(infer) => {
                match self.current_mode {
                    // sets the inferred type based on the infer type
                    InferMode::NoInfer | InferMode::Unresolved => {
                        self.current_mode = InferMode::ResolvedInfer {
                            ty: infer.to_ty(db),
                            expr: Some(CallSite::from_expr(db, expr)),
                        };
                    }
                    // inferred types have no priority over already resolved types
                    // check will be happening at the coercion level
                    InferMode::Resolved { .. } | InferMode::ResolvedInfer { .. } => {}
                }
                self.types.insert(expr, value);
            }
            Type::Elementary(elem) => match self.current_mode {
                // sets the inferred type based on the concrete type
                // the concrete type takes priority over infer types
                InferMode::NoInfer | InferMode::Unresolved | InferMode::ResolvedInfer { .. } => {
                    // both infer variants are numerics, so we can only resolve against numeric or boolean types
                    if !value.is_numeric() && !value.is_boolean() {
                        return;
                    }
                    self.current_mode = InferMode::Resolved {
                        ty: value,
                        expr: Some(CallSite::from_expr(db, expr)),
                    };
                }
                // already resolved
                InferMode::Resolved { ty, expr } => {
                    // we then perform a promotion if the size of the new type is larger
                    // todo: handle float vs int promotion
                    if value.get_size() > ty.get_size() {
                        self.current_mode = InferMode::Resolved { ty: value, expr };
                    }
                }
            },
            _ => { /*
                ignore for now:
                other types cannot be used to resolve infer variants.
                We may, however, keep that comment if we decide one day to create a richer inference system
                 */
            }
        }
    }

    pub fn get_final_type(&self) -> Type<'db> {
        match self.current_mode {
            InferMode::Resolved { ty, .. } | InferMode::ResolvedInfer { ty, .. } => ty,
            _ => Type::Never,
        }
    }

    /// Resolves completely all inferred types in the table
    pub fn resolve_completly(
        &self,
        db: &'db dyn WorkspaceDataBase,
        resolver: Resolver<'db>,
        results: &mut BodyInferenceResult<'db>,
    ) {
        // Determine the final concrete type we substitute with
        let (final_ty, source) = match self.current_mode {
            InferMode::Resolved { ty, expr } => (ty.normalize(db), expr),
            InferMode::ResolvedInfer { ty, expr } => (ty.normalize(db), expr),
            _ => {
                // no inference to resolve
                // therefore all types are left as is
                return;
            }
        };

        debug_assert!(
            !final_ty.has_infer(),
            "Final type should not be an infer type"
        );
        debug_assert!(
            matches!(final_ty, Type::Elementary(_)),
            "Final type should be an elementary type"
        );

        let elem = match final_ty {
            Type::Elementary(spec) => spec,
            _ => unreachable!("Final type should be an elementary type"),
        };

        // Insert for each expression the final resolved type
        for (expr, target_type) in &self.types {
            match target_type {
                // the inference *does not* use coercion, it just checks if the infer type can be resolved to the final type
                Type::Infer(infer) => {
                    // since a literal could either be an INT or REAL by default, we check if any could be casted to the final type
                    let cast = match infer.to_spec(db).implicit_cast(elem) {
                        // yes, therefore infer the type directly
                        Some(spec) => spec,
                        // no, so infer this type via infer_with without any cast
                        None => elem,
                    };

                    // check the literal value
                    // since we can't have infer variants, we either replace them with a concrete type or a never type
                    match infer.check_as(db, cast) {
                        Ok(typ) => {
                            results.type_of_expr.insert(*expr, Type::Elementary(cast));
                        }
                        Err(err) => {
                            let infer_str = match infer {
                                InferType::Float(fl) => fl.text(db).to_string(),
                                InferType::Integer(int) => int.ident(db).text(db).to_string(),
                            };
                            results.errors.push(
                                TypeError::InferLiteralError {
                                    expr: *expr,
                                    source,
                                    target: final_ty,
                                    err,
                                }
                                .to_diagnostic(db),
                            );
                            // set to never on error
                            // no further infer variants should be propagated
                            results.type_of_expr.insert(*expr, Type::Never);
                        }
                    }
                }
                _ => (),
            };
        }
    }
}
