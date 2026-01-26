use core::panic;

use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    CallSite, HasName, HirNodeInfo, Modifier,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError, e2_resolve::ResolveError,
        e3_type::TypeError, e5_inheritance::InheritanceError, e6_array::ArrayError,
        e7_enum::EnumError, e8_subrange::SubRangeError,
    },
    hir_def::{
        expressions::{
            expression::InitExpr,
            spec::{Array, ElementarySpec, Enum, SpecKind, Struct, StructElement, SubRange},
        },
        interned::identifier::Ident,
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body_inference::BodyInferenceResult,
        infer::expr::InferExprCtx,
        inheritance_solver::{MethodRef, inherited_methods},
        init_inference::InitExprInferenceResult,
        resolver::Resolver,
        ty::Type,
    },
};

#[tracing::instrument(skip(db))]
#[salsa::tracked(returns(ref))]
pub fn infer_signature<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
) -> PouSignature<'db> {
    PouSignature::new(scope).infer_signature(db)
}

/// Information about an element's position in an array initializer
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::Update)]
pub struct ArrayElementPosition {
    /// The dimension this element is in (0 for first dimension, etc.)
    pub dimension: usize,
    /// Number of elements this initializer fills (1 for single values, N for N(value))
    pub count: usize,
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct PouSignature<'db> {
    // Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,

    /// Main type (if any) returned by this scope (e.g., function return type)
    pub type_of_self: Option<Type<'db>>,

    /// Mapping of variables to their inferred types
    pub type_of_variables: FxHashMap<VariableDecl<'db>, Type<'db>>,

    /// Mapping of init expr to their position in array (if applicable)
    pub array_positions: FxHashMap<InitExpr<'db>, ArrayElementPosition>,

    /// Initializer expression inference results
    pub init_expr_result: InitExprInferenceResult<'db>,

    /// BodyInference results (Inference of constant expressions)
    pub body_infer_result: BodyInferenceResult<'db>,

    /// Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> PouSignature<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        Self {
            scope,
            type_of_self: None,
            type_of_variables: FxHashMap::default(),
            array_positions: FxHashMap::default(),
            init_expr_result: InitExprInferenceResult::new(scope),
            body_infer_result: BodyInferenceResult::new(scope),
            errors: Vec::new(),
        }
    }

    pub fn infer_signature(mut self, db: &'db dyn WorkspaceDataBase) -> Self {
        match get_scope(db, self.scope).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::DataType(dt) => {
                    let typ = Type::new_spec(db, dt.spec(db));
                    match dt.spec(db).kind(db) {
                        SpecKind::Array(arr) => {
                            self.infer_array(db, *arr);
                        }
                        SpecKind::Enum(enm) => {
                            self.infer_enum(db, *enm);
                        }
                        SpecKind::Subrange(subrange) => {
                            self.infer_subrange(db, *subrange);
                        }
                        SpecKind::Struct(strukt) => {
                            self.infer_struct(db, *strukt);
                        }
                        SpecKind::Target(target) => {
                            if typ.is_never() {
                                self.errors.push(
                                    ResolveError::NoNamespaceItemFound {
                                        path: target.clone(),
                                    }
                                    .to_diagnostic(db),
                                )
                            };
                        }
                        _ => {}
                    }
                    if let Some(expr) = dt.init(db) {
                        self.init_expr_result.resolve_init_expr(db, expr, typ);
                    };
                }
                _ => {}
            },
            _ => {}
        }

        self.infer_variables(db);
        self.infer_return_type(db);
        self.check_methods(db);
        self.check_inheritance(db);

        for error in &self.init_expr_result.errors {
            self.errors.push(error.clone());
        }

        for error in &self.init_expr_result.body_infer_result.errors {
            self.errors.push(error.clone());
        }

        for error in &self.body_infer_result.errors {
            self.errors.push(error.clone());
        }

        self
    }

    fn infer_variables(&mut self, db: &'db dyn WorkspaceDataBase) {
        let variables = match self.scope.variables(db) {
            Some(vars) => vars,
            None => return,
        };

        let mut seen = FxHashMap::default();
        for var in variables.iter() {
            match seen.get(&var.get_name_ident(db)) {
                Some(prev) => {
                    self.errors.push(
                        DuplicateError::Variable {
                            var1: *var,
                            var2: *prev,
                        }
                        .to_diagnostic(db),
                    );
                }
                None => {
                    seen.insert(var.get_name_ident(db), *var);
                }
            }

            let var_type = Type::new_spec(db, var.spec(db));
            if var_type.is_never() {
                if let SpecKind::Target(target) = var.spec(db).kind(db) {
                    self.errors.push(
                        ResolveError::NoNamespaceItemFound {
                            path: target.clone(),
                        }
                        .to_diagnostic(db),
                    );
                    self.type_of_variables.insert(*var, Type::Never);
                } else {
                    self.type_of_variables.insert(*var, var_type);
                }
            }

            if let Some(init_expr) = var.init(db) {
                self.init_expr_result
                    .resolve_init_expr(db, init_expr, var_type);
            }
        }
    }

    fn infer_return_type(&mut self, db: &'db dyn WorkspaceDataBase) {
        let return_typ = match get_scope(db, self.scope).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => f.return_type(db).copied(),
                _ => None,
            },
            ScopeKind::MethodDecl(m) => m.return_type(db).copied(),
            _ => None,
        };

        if let Some(ret_type) = return_typ {
            let typ = Type::new_spec(db, ret_type);
            match typ {
                Type::Infer(_) | Type::Never => {
                    if let SpecKind::Target(target) = ret_type.kind(db) {
                        self.errors.push(
                            ResolveError::NoNamespaceItemFound {
                                path: target.clone(),
                            }
                            .to_diagnostic(db),
                        );
                        self.type_of_self = Some(Type::Never);
                    }
                }
                _ => {
                    self.type_of_self = Some(typ);
                }
            }
        }
    }

    fn check_inheritance(&mut self, db: &'db dyn WorkspaceDataBase) {
        let implementer = match get_scope(db, self.scope).kind {
            ScopeKind::Pou(pou) => pou,
            _ => return,
        };

        let declared_methods = &implementer.get_scope_id(db).def_map(db).declared_methods;
        let inherited_methods = inherited_methods(db, implementer);

        if let Pou::Class(cl) = implementer {
            // If the class is abstract, it must have at least one abstract method
            if cl.modifier(db).contains(Modifier::ABSTRACT)
                && !declared_methods
                    .iter()
                    .any(|(_, m)| m.modifier(db).contains(Modifier::ABSTRACT))
            {
                self.errors.push(
                    InheritanceError::AbstractClassHasNoAbstractMethods { class: implementer }
                        .to_diagnostic(db),
                );
            };
        };

        // check dups in inherited methods
        for (m1, m2) in &inherited_methods.duplicates {
            self.errors.push(
                DuplicateError::InheritedMethod {
                    method1: *m1,
                    method2: *m2,
                }
                .to_diagnostic(db),
            );
        }

        // check unresolved
        for unresolved in &inherited_methods.unresolved {
            self.errors.push(
                ResolveError::NoNamespaceItemFound {
                    path: unresolved.clone(),
                }
                .to_diagnostic(db),
            );
        }

        // look at the inherited methods first
        for (inherited_name, inherited_method) in inherited_methods.methods.iter() {
            let inherited_method = inherited_method.method;
            // method is inherited from a base interface/class
            if let Some(declared_method) = declared_methods.get(inherited_name) {
                check_signature(db, inherited_method, *declared_method, &mut self.errors);

                match (inherited_method.modifier(db), declared_method.modifier(db)) {
                    // Override of a final method
                    (Modifier::FINAL, Modifier::OVERRIDE) => {
                        self.errors.push(
                            InheritanceError::OverrideFinalMethod {
                                base_method: inherited_method,
                                derived_method: *declared_method,
                            }
                            .to_diagnostic(db),
                        );
                    }
                    // Override of method without override
                    (_, Modifier::EMPTY) => {
                        self.errors.push(
                            InheritanceError::MissingOverride {
                                base_method: inherited_method,
                                derived_method: *declared_method,
                            }
                            .to_diagnostic(db),
                        );
                    }
                    _ => {}
                }
            } else {
                // inherited method is not present

                // method is from an interface
                if inherited_method.is_prototype() {
                    self.errors.push(
                        InheritanceError::UnimplementedInterfaceMethod {
                            implementer,
                            method: inherited_method,
                        }
                        .to_diagnostic(db),
                    );
                }

                if let Modifier::ABSTRACT = inherited_method.modifier(db) {
                    self.errors.push(
                        InheritanceError::MissingAbstractMethod {
                            implementer,
                            base_method: inherited_method,
                        }
                        .to_diagnostic(db),
                    );
                }
            }
        }
        // Look at the declared methods

        for (base_name, base_method) in declared_methods {
            if inherited_methods.methods.contains_key(base_name) {
            } else if base_method.modifier(db) == Modifier::OVERRIDE {
                self.errors.push(
                    InheritanceError::EmptyOverride {
                        base_method: *base_method,
                    }
                    .to_diagnostic(db),
                );
            }
        }
    }

    fn check_methods(&mut self, db: &'db dyn WorkspaceDataBase) {
        if let Some(methods) = self.scope.method_declarations(db) {
            let mut seen = FxHashMap::default();
            for method in methods.iter() {
                match seen.get(&method.name(db)) {
                    Some(prev) => {
                        self.errors.push(
                            DuplicateError::MethodDecl {
                                method1: *method,
                                method2: *prev,
                            }
                            .to_diagnostic(db),
                        );
                    }
                    None => {
                        seen.insert(method.name(db), *method);
                    }
                }
            }
        };

        if let Some(prototypes) = self.scope.method_prototypes(db) {
            let mut seen_prots = FxHashMap::default();
            for method in prototypes.iter() {
                match seen_prots.get(&method.name(db)) {
                    Some(prev) => self.errors.push(
                        DuplicateError::MethodProt {
                            method1: *method,
                            method2: *prev,
                        }
                        .to_diagnostic(db),
                    ),
                    None => {
                        seen_prots.insert(method.name(db), *method);
                    }
                }
            }
        };
    }

    fn infer_array(&mut self, db: &'db dyn WorkspaceDataBase, array: Array<'db>) {
        for range in array.subranges(db) {
            let lower = range.0;
            let upper = range.1;

            match (lower.as_range(db), upper.as_range(db)) {
                (Some(lower_range), Some(upper_range)) => {
                    if lower_range > upper_range {
                        self.errors.push(
                            ArrayError::InferiorUpperBound {
                                lower: lower_range,
                                upper: upper_range,
                                upper_expr: upper,
                            }
                            .to_diagnostic(db),
                        );
                    }
                }
                (None, _) => {
                    self.errors.push(
                        ArrayError::InvalidArrayLowerValue { value: lower }.to_diagnostic(db),
                    );
                }
                (_, None) => {
                    self.errors.push(
                        ArrayError::InvalidArrayUpperValue { value: upper }.to_diagnostic(db),
                    );
                }
            }
        }
    }

    fn infer_enum(&mut self, db: &'db dyn WorkspaceDataBase, enm: Enum<'db>) {
        if let Some(spec) = enm.typ(db) {
            let typ = Type::new_spec(db, spec);
            match typ {
                Type::Elementary(elementary) => match elementary {
                    ElementarySpec::Byte
                    | ElementarySpec::Word
                    | ElementarySpec::DWord
                    | ElementarySpec::LWord
                    | ElementarySpec::SInt
                    | ElementarySpec::USInt
                    | ElementarySpec::Int
                    | ElementarySpec::UInt
                    | ElementarySpec::DInt
                    | ElementarySpec::UDInt
                    | ElementarySpec::LInt
                    | ElementarySpec::ULInt => {}
                    _ => self
                        .errors
                        .push(EnumError::InvalidEnumType { value: spec, typ }.to_diagnostic(db)),
                },
                _ => self
                    .errors
                    .push(EnumError::InvalidEnumType { value: spec, typ }.to_diagnostic(db)),
            }
        };

        let mut seen = FxHashMap::default();
        for variant in &enm.variants(db) {
            // Check duplicate variant names
            match seen.get(&variant.name.ident) {
                Some(prev) => self.errors.push(
                    DuplicateError::EnumVariant {
                        variant1: *prev,
                        variant2: variant.name,
                    }
                    .to_diagnostic(db),
                ),
                None => {
                    seen.insert(variant.name.ident, variant.name);
                }
            }

            // Check variant value type
            if let (Some(value), Some(typ)) = (variant.value, enm.typ(db)) {
                let resolver = Resolver::for_scope(db, value.scope_id(db));
                let mut infer = InferExprCtx::new(resolver);

                let target = Type::new_spec(db, typ);
                infer.resolve_expr(db, value, &mut self.body_infer_result);
                infer.check_expr(db, value, &mut self.body_infer_result);

                if let Err(err) =
                    infer.coerce_type_with_expr(db, target, value, &mut self.body_infer_result)
                {
                    self.errors.push(
                        TypeError::NotAssignable {
                            base_target: target,
                            lhs: err.expected,
                            rhs: err.actual,
                            adjustment: err.adjustment,
                            expr: CallSite::from_expr(db, value),
                        }
                        .to_diagnostic(db),
                    )
                }
            }
        }
    }

    fn infer_subrange(&mut self, db: &'db dyn WorkspaceDataBase, subrange: SubRange<'db>) {
        let typ = Type::new_spec(db, subrange._type(db));

        match typ {
            Type::Elementary(elementary) => match elementary {
                ElementarySpec::Byte
                | ElementarySpec::Word
                | ElementarySpec::DWord
                | ElementarySpec::LWord
                | ElementarySpec::SInt
                | ElementarySpec::USInt
                | ElementarySpec::Int
                | ElementarySpec::UInt
                | ElementarySpec::DInt
                | ElementarySpec::UDInt
                | ElementarySpec::LInt
                | ElementarySpec::ULInt => {}
                _ => {
                    self.errors.push(
                        SubRangeError::InvalidSubrangeType {
                            spec: subrange._type(db),
                            typ,
                        }
                        .to_diagnostic(db),
                    );
                    return;
                }
            },
            _ => {
                self.errors.push(
                    SubRangeError::InvalidSubrangeType {
                        spec: subrange._type(db),
                        typ,
                    }
                    .to_diagnostic(db),
                );
                return;
            }
        }

        let min = subrange.lower(db);
        let max = subrange.upper(db);

        let resolver = Resolver::for_scope(db, subrange.lower(db).scope_id(db));
        let mut infer = InferExprCtx::new(resolver);

        infer.resolve_expr(db, min, &mut self.body_infer_result);
        infer.resolve_expr(db, max, &mut self.body_infer_result);
        infer.check_expr(db, min, &mut self.body_infer_result);
        infer.check_expr(db, max, &mut self.body_infer_result);

        if let Err(err) = infer.coerce_type_with_expr(db, typ, min, &mut self.body_infer_result) {
            self.errors.push(
                TypeError::NotAssignable {
                    base_target: typ,
                    lhs: err.expected,
                    rhs: err.actual,
                    adjustment: err.adjustment,
                    expr: CallSite::from_expr(db, min),
                }
                .to_diagnostic(db),
            )
        }

        if let Err(err) = infer.coerce_type_with_expr(db, typ, max, &mut self.body_infer_result) {
            self.errors.push(
                TypeError::NotAssignable {
                    base_target: typ,
                    lhs: err.expected,
                    rhs: err.actual,
                    adjustment: err.adjustment,
                    expr: CallSite::from_expr(db, max),
                }
                .to_diagnostic(db),
            )
        }
    }

    fn infer_struct(&mut self, db: &'db dyn WorkspaceDataBase, strukt: Struct<'db>) {
        let mut seen: FxHashMap<Ident, StructElement> = FxHashMap::default();
        for field in &strukt.elements(db) {
            match seen.get(&field.get_name_ident(db)) {
                Some(prev) => {
                    self.errors.push(
                        DuplicateError::StructField {
                            field1: *field,
                            field2: *prev,
                        }
                        .to_diagnostic(db),
                    );
                }
                None => {
                    seen.insert(field.get_name_ident(db), *field);
                }
            }
        }
    }
}

fn check_signature<'db>(
    db: &'db dyn WorkspaceDataBase,
    m1: MethodRef<'db>,
    m2: MethodRef<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let sig1 = m1.variables(db);
    let sig2 = m2.variables(db);
    if sig1.len() != sig2.len() {
        errors.push(
            InheritanceError::SignatureParametersCountMismatch {
                m1,
                expected: sig1.len(),
                m2,
                got: sig2.len(),
            }
            .to_diagnostic(db),
        );
    }

    for (var1, var2) in sig1.iter().zip(sig2.iter()) {
        let var1_typ = Type::new_var(db, *var1);
        let var2_typ = Type::new_var(db, *var2);

        if !var1_typ.normalize(db).eq(&var2_typ.normalize(db)) {
            errors.push(
                InheritanceError::SignatureTypeMismatch {
                    expected: var1_typ,
                    got: var2_typ,
                    method: m1,
                }
                .to_diagnostic(db),
            )
        }
    }
}
