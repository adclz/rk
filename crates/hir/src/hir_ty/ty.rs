use std::sync::Arc;

use auto_lsp::{core::span::Span, default::db::BaseDatabase};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    AstId, HirNodeInfo, TypeInfo,
    check::errors::path_error::PathResolveError,
    hir_def::{
        expressions::{
            expression::Expr,
            spec::{ElementarySpec, Enum, Spec, SpecKind, StructElement},
        },
        interned::{identifier::Ident, namespace::SpanNamespaceAccess},
        modifier::Modifier,
        pous::{
            class::MethodDecl,
            interface::MethodPrototype,
            pou::{Pou, PouDecl},
            variable::{VariableDecl, VariableKind},
        },
        scope::FileScopeId,
        visibility::Visibility,
    },
    hir_ty::{
        inheritance_solver::method_table, name_res::resolve_namespace_access,
        ty_var_access_resolver::PathExprWalkStep,
    },
};

/// The  resolved type of a variable, POU, or method
/// [`Ty`] is the most fundamental unit of type information in the HIR.
#[salsa::tracked(debug)]
pub struct Ty<'db> {
    // Where the type is declared (POU, Variable, Method)
    //pub decl: TyDecl<'db>,

    // Where the type is defined (POU, Spec, Method)
    pub def: TyDef<'db>,

    // The actual kind of the type
    #[tracked]
    #[returns(ref)]
    pub kind: TyKind<'db>,
}

impl<'db> Ty<'db> {
    pub fn name(&self, db: &'db dyn BaseDatabase) -> String {
        self.def(db).name(db)
    }

    pub fn name_span(&self, db: &'db dyn BaseDatabase) -> Span {
        match self.def(db) {
            TyDef::Pou(pou) => pou.get_name_span(db).unwrap(),
            TyDef::Method(method) => method.get_name_span(db).unwrap(),
            TyDef::MethodProt(method) => method.get_name_span(db).unwrap(),
            TyDef::Spec(spec) => spec.get_span(db),
        }
    }
}

impl<'db> HirNodeInfo<'db> for Ty<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.def(db).get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.def(db).get_scope_id(db)
    }
}

// Definition of the type (POU or Spec)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyDef<'db> {
    Pou(PouDecl<'db>),
    Method(MethodDecl<'db>),
    MethodProt(MethodPrototype<'db>),
    Spec(Spec<'db>),
}

impl<'db> TyDef<'db> {
    pub fn name(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            TyDef::Pou(pou) => pou.name(db).text(db).to_string(),
            TyDef::Method(method) => method.name(db).text(db).to_string(),
            TyDef::MethodProt(method) => method.name(db).text(db).to_string(),
            TyDef::Spec(spec) => spec.shorthand(db),
        }
    }
    pub fn def_as_ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        match self {
            Self::Pou(pou) => Some(ty_for_pou(db, *pou)),
            Self::Method(method) => Some(ty_for_method_decl(db, *method)),
            Self::MethodProt(method) => Some(ty_for_method_prot(db, *method)),
            _ => None,
        }
    }

    pub fn modifier(&self, db: &'db dyn BaseDatabase) -> Modifier {
        match self {
            TyDef::Method(m) => m.modifier(db),
            TyDef::Pou(p) => p.modifier(db),
            _ => Modifier::empty(),
        }
    }

    pub fn is_pou(&self) -> bool {
        matches!(self, TyDef::Pou(_))
    }

    pub fn is_method(&self) -> bool {
        matches!(self, TyDef::Method(_))
    }

    pub fn is_spec(&self) -> bool {
        matches!(self, TyDef::Spec(_))
    }

    pub fn get_span(&self, db: &'db dyn BaseDatabase) -> Option<Span> {
        match self {
            TyDef::Pou(pou) => Some(pou.get_span(db)),
            TyDef::Method(method) => Some(method.get_span(db)),
            TyDef::MethodProt(method) => Some(method.get_span(db)),
            TyDef::Spec(spec) => Some(spec.get_span(db)),
        }
    }
}

impl<'db> HirNodeInfo<'db> for TyDef<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            TyDef::Pou(pou) => pou.get_id(db),
            TyDef::Method(method) => method.get_id(db),
            TyDef::MethodProt(method) => method.get_id(db),
            TyDef::Spec(spec) => spec.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self {
            TyDef::Pou(pou) => pou.get_scope_id(db),
            TyDef::Method(method) => method.get_scope_id(db),
            TyDef::MethodProt(method) => method.get_scope_id(db),
            TyDef::Spec(spec) => spec.get_scope_id(db),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum TyKind<'db> {
    // Literal types
    Simple(ElementarySpec),
    Enum {
        typ: Option<Spec<'db>>,
        spec: Enum<'db>,
    },
    SubRange {
        typ: Spec<'db>,
        min: Expr<'db>,
        max: Expr<'db>,
    },
    RefTo(Spec<'db>),
    Array {
        ranges: Vec<(Expr<'db>, Expr<'db>)>,
        typ: Spec<'db>,
    },
    ArrayConformand {
        typ: Spec<'db>,
    },

    Struct {
        spec: Spec<'db>,
        elements: FxHashMap<Ident, StructElement<'db>>,
    },

    Interface {
        implements: Vec<PouDecl<'db>>,
        methods: Vec<MethodPrototype<'db>>,
    },

    Class {
        extends: Option<PouDecl<'db>>,
        implements: Vec<PouDecl<'db>>,
        methods: Vec<MethodDecl<'db>>,
    },

    Function {
        return_type: Option<Spec<'db>>,
    },

    FunctionBlock {
        extends: Option<PouDecl<'db>>,
        methods: Vec<MethodDecl<'db>>,
    },

    Method {
        is_prototype: bool,
        return_type: Option<Spec<'db>>,
    },

    // Error variants
    Unresolved(SpanNamespaceAccess<'db>),
}

#[tracing::instrument(skip_all, name = "query_type_signature")]
#[salsa::tracked]
pub fn ty_for_pou<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Ty<'db> {
    let def = TyDef::Pou(pou);

    match pou.pou(db) {
        Pou::Function(func) => Ty::new(
            db,
            def,
            TyKind::Function {
                return_type: func.return_type(db).copied(),
            },
        ),
        Pou::FunctionBlock(fb) => {
            let extends = fb
                .extends(db)
                .and_then(|e| resolve_namespace_access(db, e.get_scope_id(db), e.path));

            let methods = fb.methods(db).to_vec();

            Ty::new(db, def, TyKind::FunctionBlock { extends, methods })
        }
        Pou::DataType(dt) => dt.spec(db).spec_to_ty(db),
        Pou::Class(class) => {
            let mut class_methods = vec![];

            for m in class.methods(db) {
                class_methods.push(*m);
            }

            let extends = class
                .extends(db)
                .and_then(|e| resolve_namespace_access(db, e.scope_id, e.path));

            let implements = class
                .implements(db)
                .iter()
                .filter_map(|interface| {
                    resolve_namespace_access(db, interface.scope_id, interface.path)
                })
                .collect();

            Ty::new(
                db,
                def,
                TyKind::Class {
                    extends,
                    implements,
                    methods: class_methods,
                },
            )
        }
        Pou::Interface(interface) => {
            let mut interface_methods = vec![];

            for m in interface.methods(db) {
                interface_methods.push(*m);
            }

            let implements = interface
                .extends(db)
                .map(|e| {
                    e.iter()
                        .filter_map(|interface| {
                            resolve_namespace_access(db, interface.scope_id, interface.path)
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ty::new(
                db,
                def,
                TyKind::Interface {
                    implements,
                    methods: interface_methods,
                },
            )
        }
    }
}

#[salsa::tracked]
pub fn ty_for_method_decl<'db>(db: &'db dyn BaseDatabase, method: MethodDecl<'db>) -> Ty<'db> {
    let def = TyDef::Method(method);

    Ty::new(
        db,
        def,
        TyKind::Method {
            is_prototype: false,
            return_type: method.return_type(db).copied(),
        },
    )
}

#[salsa::tracked]
pub fn ty_for_method_prot<'db>(db: &'db dyn BaseDatabase, method: MethodPrototype<'db>) -> Ty<'db> {
    let def = TyDef::MethodProt(method);

    Ty::new(
        db,
        def,
        TyKind::Method {
            is_prototype: true,
            return_type: method.return_type(db).copied(),
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchMode {
    Local,
    Global,
}

#[salsa::tracked]
impl<'db> Ty<'db> {
    pub fn walk(
        &self,
        db: &'db dyn BaseDatabase,
        step: &PathExprWalkStep<'db>,
        search_mode: SearchMode,
    ) -> Result<Ty<'db>, PathResolveError<'db>> {
        match &step {
            PathExprWalkStep::Field { ident, expr } => {
                // Try different lookup strategies in order, falling through to the next if not found
                
                // first, try struct fields (highest priority)
                if let TyKind::Struct { elements, .. } = self.kind(db) {
                    if let Some(field) = elements.get(&ident.ident) {
                        return Ok(field.spec(db).spec_to_ty(db));
                    }
                }
                
                // try methods for Class and FunctionBlock types
                match self.kind(db) {
                    TyKind::Class { .. } | TyKind::FunctionBlock { .. } => {
                        if let Some(method) = method_table(db, *self).declared_methods.get(&ident.ident) {
                            return Ok(method.clone());
                        }
                        // Fall through to variable lookup
                    }
                    _ => {}
                }
                
                // arrays cannot have fields (return error immediately)
                if matches!(self.kind(db), TyKind::Array { .. }) {
                    return Err(PathResolveError::TypeHasNoField {
                        ty: *self,
                        expr: *expr,
                    });
                }

                if matches!(self.kind(db), TyKind::RefTo(_)) {
                    return Err(PathResolveError::MissingDeref {
                        ty: *self,
                        expr: *expr,
                    });
                }
                
                // last resort: look for variables
                match search_mode {
                    SearchMode::Global => self.all_variables(db).get(&ident.ident).cloned(),
                    SearchMode::Local => self.local_variables(db).get(&ident.ident).cloned(),
                }
                .ok_or(PathResolveError::UnknownField {
                    expr: *expr,
                    ty: *self,
                })
            }
            PathExprWalkStep::Index { expr } => match self.kind(db) {
                TyKind::Array { typ, .. } => Ok(typ.spec_to_ty(db)),
                _ => Err(PathResolveError::NotAnArray {
                    expr: *expr,
                    ty: *self,
                }),
            },
            PathExprWalkStep::Deref { expr, target } => {
                let target = match self.kind(db) {
                    // Look for a struct field
                    TyKind::Struct { elements, spec } => elements
                        .get(&target.ident)
                        .map(|f| f.spec(db).spec_to_ty(db))
                        .ok_or(PathResolveError::UnknownField {
                            expr: *expr,
                            ty: *self,
                        }),
                    // Otherwise, look for a variable
                    _ => match search_mode {
                        SearchMode::Global => self.all_variables(db).get(&target.ident).cloned(),
                        SearchMode::Local => self.local_variables(db).get(&target.ident).cloned(),
                    }
                    .ok_or(PathResolveError::UnknownField {
                        expr: *expr,
                        ty: *self,
                    }),
                }?;

                match target.kind(db) {
                    TyKind::RefTo(inner) => Ok(inner.spec_to_ty(db)),
                    _ => Err(PathResolveError::NotAReference {
                        expr: *expr,
                        ty: *self,
                    }),
                }
            }
        }
    }

    pub fn inheritors(&self, db: &'db dyn BaseDatabase) -> Vec<Ty<'db>> {
        match self.kind(db) {
            TyKind::Class {
                extends,
                implements,
                ..
            } => {
                let mut inheritors = vec![];
                if let Some(base) = extends {
                    inheritors.push(ty_for_pou(db, *base));
                }
                for iface in implements {
                    inheritors.push(ty_for_pou(db, *iface));
                }
                inheritors
            }
            TyKind::Interface { implements, .. } => {
                let mut inheritors = vec![];
                for iface in implements {
                    inheritors.push(ty_for_pou(db, *iface));
                }
                inheritors
            }
            TyKind::FunctionBlock { extends, .. } => {
                let mut inheritors = vec![];
                if let Some(base) = extends {
                    inheritors.push(ty_for_pou(db, *base));
                }
                inheritors
            }
            _ => vec![],
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn all_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, Ty<'db>> {
        match self.kind(db) {
            _ => match self.def(db) {
                TyDef::Pou(pou) => match pou.pou(db) {
                    Pou::Function(f) => self.fetch_all_variables(db, &f.variables(db)),
                    Pou::FunctionBlock(fb) => self.fetch_all_variables(db, &fb.variables(db)),
                    _ => Default::default(),
                },
                TyDef::Method(method) => self.fetch_all_variables(db, &method.variables(db)),
                TyDef::MethodProt(method) => self.fetch_all_variables(db, &method.variables(db)),
                _ => Default::default(),
            },
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn local_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, Ty<'db>> {
        match self.kind(db) {
            _ => match self.def(db) {
                TyDef::Pou(pou) => match pou.pou(db) {
                    Pou::Function(f) => self.fetch_local_variables(db, &f.variables(db)),
                    Pou::FunctionBlock(fb) => self.fetch_local_variables(db, &fb.variables(db)),
                    _ => Default::default(),
                },
                TyDef::Method(method) => self.fetch_local_variables(db, &method.variables(db)),
                TyDef::MethodProt(method) => self.fetch_local_variables(db, &method.variables(db)),
                _ => Default::default(),
            },
        }
    }

    fn fetch_local_variables(
        &self,
        db: &'db dyn BaseDatabase,
        vars: &[VariableDecl<'db>],
    ) -> IndexMap<Ident, Ty<'db>> {
        let mut variables = IndexMap::default();
        for v in vars {
            match v.kind(db) {
                VariableKind::Input => {
                    variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
                }
                VariableKind::Output => {
                    variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
                }
                VariableKind::InOut => {
                    variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
                }
                _ => continue,
            };
        }
        variables
    }

    fn fetch_all_variables(
        &self,
        db: &'db dyn BaseDatabase,
        vars: &[VariableDecl<'db>],
    ) -> IndexMap<Ident, Ty<'db>> {
        let mut variables = IndexMap::default();
        for v in vars {
            variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
        }
        variables
    }

    pub fn visibility(&self, db: &'db dyn BaseDatabase) -> Option<Visibility> {
        match self.def(db) {
            TyDef::Method(method) => Some(method.visibility(db)),
            _ => None,
        }
    }

    pub fn is_simple(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Simple(_))
    }

    pub fn is_callable(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(
            self.kind(db),
            TyKind::Function { .. } | TyKind::FunctionBlock { .. } | TyKind::Method { .. }
        )
    }

    pub fn is_method_prototype(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::Method { is_prototype, .. } = self.kind(db) {
            return *is_prototype;
        }
        false
    }

    pub fn is_unresolved(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Unresolved(_))
    }

    pub fn is_reference(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::RefTo(_))
    }

    pub fn has_return_type(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        match self.kind(db) {
            TyKind::Function { return_type } => return_type.map(|rt| rt.spec_to_ty(db)),
            TyKind::Method { return_type, .. } => return_type.map(|rt| rt.spec_to_ty(db)),
            _ => None,
        }
    }
}

#[salsa::tracked]
impl<'db> Spec<'db> {
    #[salsa::tracked]
    pub fn spec_to_ty(self, db: &'db dyn BaseDatabase) -> Ty<'db> {
        let kind = match self.kind(db) {
            SpecKind::Array(array) => TyKind::Array {
                typ: *array.of_type,
                ranges: array
                    .subranges
                    .iter()
                    .map(|(start, end)| (*start, *end))
                    .collect(),
            },
            SpecKind::Enum(enum_spec) => TyKind::Enum {
                typ: enum_spec.typ,
                spec: enum_spec.clone(),
            },
            SpecKind::Subrange(subrange) => TyKind::SubRange {
                typ: *subrange._type,
                min: subrange.lower,
                max: subrange.upper,
            },
            SpecKind::Struct(fields) => TyKind::Struct {
                spec: self,
                elements: fields
                    .elements
                    .iter()
                    .map(|element| (*element.name(db), *element))
                    .collect(),
            },
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, self.scope_id(db), target.path) {
                    Some(pou) => ty_for_pou(db, pou).kind(db).clone(),
                    None => TyKind::Unresolved(*target),
                }
            }
            SpecKind::Simple(simple) => TyKind::Simple(*simple),
            SpecKind::ArrayConformand(array) => TyKind::ArrayConformand { typ: *array },
            SpecKind::Ref(_ref) => TyKind::RefTo(*_ref),
        };
        Ty::new(db, TyDef::Spec(self), kind)
    }
}

impl<'db> TypeInfo<'db> for Ty<'db> {
    fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            TyKind::Simple(elem) => elem.type_name(db),
            TyKind::Enum { .. } => "ENUM".into(),
            TyKind::SubRange { .. } => "SUBRANGE".into(),
            TyKind::RefTo(ref_) => format!("REF_TO {}", ref_.spec_to_ty(db).type_name(db)),
            TyKind::Array { .. } => "ARRAY".into(),
            TyKind::ArrayConformand { .. } => "ARRAY*".into(),
            TyKind::Struct { .. } => "STRUCT".into(),
            TyKind::Interface { .. } => "INTERFACE".into(), 
            TyKind::Class { .. } => "CLASS".into(),
            TyKind::Function { .. } => "FUNCTION".into(),
            TyKind::FunctionBlock { .. } => "FUNCTION_BLOCK".into(),
            TyKind::Method { .. } => "METHOD".into(),
            TyKind::Unresolved(_) => "{unknown}".into(),
        }
    }
}
