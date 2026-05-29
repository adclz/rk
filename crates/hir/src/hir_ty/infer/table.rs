use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    CallSite,
    check::errors::{ToIdeDiagnostic, e3_type::TypeError},
    hir_def::expressions::{expression::Expr, spec::ElementarySpec},
    hir_ty::{body::BodyInferenceResult, infer::Infer, resolver::Resolver, ty::Type},
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

// Source of the inference, used for error reporting
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InferSource<'db> {
    Type(Type<'db>),
    CallSite(CallSite<'db>),
}

impl<'db> From<Type<'db>> for InferSource<'db> {
    fn from(typ: Type<'db>) -> Self {
        InferSource::Type(typ)
    }
}

impl<'db> From<CallSite<'db>> for InferSource<'db> {
    fn from(callsite: CallSite<'db>) -> Self {
        InferSource::CallSite(callsite)
    }
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
        expr: Option<InferSource<'db>>,
    },
    // Fully resolved type
    // Resolved has a priority over ResolvedInfer
    Resolved {
        ty: Type<'db>,
        expr: Option<InferSource<'db>>,
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
        expr: Option<InferSource<'db>>,
        ty: Type<'db>,
    ) {
        // only elementary types can be used to resolve the inference
        // the inference table will only attempt to resolve infer variants, it does not check coercion
        //
        // Function/Method names used as return values (e.g. CHK_REAL := 16#00)
        // need to resolve through the return type.
        let ty = match ty {
            Type::Function(_) | Type::MethodDecl(_) => match ty.with_return_type(db) {
                Some(ret) => ret.normalize(db),
                None => return,
            },
            _ => ty,
        };
        let normalized = ty.normalize(db);
        let ty = match normalized {
            Type::SubRange(sub) => sub._type(db).infer(db),
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
                            expr: Some(CallSite::from_scoped(db, &expr).into()),
                        };
                    }
                    // inferred types have no priority over already resolved types
                    // check will be happening at the coercion level
                    InferMode::Resolved { .. } | InferMode::ResolvedInfer { .. } => {}
                }
                self.types.insert(expr, value);
            }
            Type::Elementary(elem) => match &self.current_mode {
                // sets the inferred type based on the concrete type
                // the concrete type takes priority over infer types
                InferMode::NoInfer | InferMode::Unresolved | InferMode::ResolvedInfer { .. } => {
                    // both infer variants are numerics, so we can only resolve against numeric or boolean types
                    if !value.is_numeric() && !value.is_boolean() {
                        return;
                    }
                    self.current_mode = InferMode::Resolved {
                        ty: value,
                        expr: Some(CallSite::from_scoped(db, &expr).into()),
                    };
                }
                // already resolved
                InferMode::Resolved { ty, expr } => {
                    // Promote to the wider of the two *only if* implicit
                    // widening exists between them. Using raw byte size
                    // misclassified cross-category pairs (e.g. BYTE + REAL
                    // where REAL is "wider" in bytes but not in the IEC cast
                    // table), which caused `ROR(BYTE#0, 0.8)` - an already
                    // invalid call - to spuriously resolve the return to REAL
                    // and produce a cascading assignment error.
                    //
                    // `elem.implicit_cast(ty_elem)` returns Some when ty_elem
                    // widens to elem, i.e. elem is the larger type. Equal
                    // types and fully incompatible pairs both return None,
                    // meaning we keep the earlier resolution in place.
                    if let Type::Elementary(ty_elem) = ty
                        && elem.implicit_cast(*ty_elem).is_some()
                    {
                        self.current_mode = InferMode::Resolved {
                            ty: value,
                            expr: *expr,
                        };
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

        match final_ty {
            Type::Elementary(spec) => {
                self.resolve_with_elementary_spec(db, final_ty, source, spec, resolver, results)
            }
            _ => unreachable!("Final type should be an elementary type"),
        };
    }

    fn resolve_with_elementary_spec(
        &self,
        db: &'db dyn WorkspaceDataBase,
        final_ty: Type<'db>,
        source: Option<InferSource<'db>>,
        elem: ElementarySpec,
        resolver: Resolver<'db>,
        results: &mut BodyInferenceResult<'db>,
    ) {
        // Insert for each expression the final resolved type. The
        // inference *does not* use coercion — it just checks that the
        // Infer type can be resolved to the final concrete type.
        for (expr, target_type) in &self.types {
            if let Type::Infer(infer) = target_type {
                // Check the literal value against the target type directly:
                // bare literals adapt to the target, `check_as` validates
                // the value range.
                match infer.check_as(db, elem) {
                    Ok(typ) => {
                        results.type_of_expr.insert(*expr, Type::Elementary(elem));
                    }
                    Err(err) => {
                        results.errors.push(
                            TypeError::InferLiteralError {
                                expr: *expr,
                                source,
                                target: final_ty,
                                err,
                            }
                            .to_diagnostic(db, results.scope.file(db)),
                        );

                        // Necessary: the Infer variant MUST be replaced by Type::Never
                        results.type_of_expr.insert(*expr, Type::Never);
                    }
                }
            }
        }
    }
}
