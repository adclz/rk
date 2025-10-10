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
        ty_path_expr_resolver::PathExprWalkStep,
    },
};

/// The  resolved type of a variable, POU, or method
/// [`Ty`] is the most fundamental unit of type information in the HIR.
#[salsa::tracked(debug)]
pub struct Ty<'db> {
    // Where the type is declared (POU, Variable, Method)
    pub decl: TyDecl<'db>,

    // Where the type is defined (POU, Spec, Method)
    pub def: TyDef<'db>,

    // The actual kind of the type
    #[tracked]
    #[no_eq]
    #[returns(ref)]
    pub kind: TyKind<'db>,
}

impl<'db> HirNodeInfo<'db> for Ty<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.decl(db).get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.decl(db).scope_id(db)
    }
}

// Declaration of the type (POU, Variable, Method)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum TyDecl<'db> {
    Pou(PouDecl<'db>),
    Variable(VariableDecl<'db>),
    Method(MethodDecl<'db>),
    MethodProt(MethodPrototype<'db>),
    StructElement(StructElement<'db>),
}

impl<'db> TyDecl<'db> {
    pub fn decl_as_ty(&self, db: &'db dyn BaseDatabase) -> Ty<'db> {
        match self {
            TyDecl::Pou(pou) => ty_for_pou(db, *pou),
            TyDecl::Variable(variable) => ty_for_variable(db, *variable),
            TyDecl::Method(method) => ty_for_method_decl(db, *method),
            TyDecl::MethodProt(method) => ty_for_method_prot(db, *method),
            TyDecl::StructElement(element) => ty_for_struct_field(db, *element),
        }
    }

    pub fn span(&self, db: &'db dyn BaseDatabase) -> Span {
        match self {
            TyDecl::Pou(pou) => pou.get_span(db),
            TyDecl::Variable(variable) => variable.get_span(db),
            TyDecl::Method(method) => method.get_span(db),
            TyDecl::MethodProt(method) => method.get_span(db),
            TyDecl::StructElement(element) => element.get_span(db),
        }
    }

    pub fn name_span(&self, db: &'db dyn BaseDatabase) -> Span {
        match self {
            TyDecl::Pou(pou) => pou.get_name_span(db),
            TyDecl::Variable(variable) => variable.get_name_span(db),
            TyDecl::Method(method) => method.get_name_span(db),
            TyDecl::MethodProt(method) => method.get_name_span(db),
            TyDecl::StructElement(element) => element.get_name_span(db),
        }
        .unwrap_or_else(|| panic!("All TyDecl variants should have a name span: {self:?}"))
    }

    pub fn name(&self, db: &'db dyn BaseDatabase) -> Ident {
        match self {
            TyDecl::Pou(pou) => *pou.name(db),
            TyDecl::Variable(variable) => *variable.name(db),
            TyDecl::Method(method) => *method.name(db),
            TyDecl::MethodProt(method) => *method.name(db),
            TyDecl::StructElement(element) => *element.name(db),
        }
    }

    pub fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            TyDecl::Pou(pou) => pou.get_id(db),
            TyDecl::Variable(variable) => variable.get_id(db),
            TyDecl::Method(method) => method.get_id(db),
            TyDecl::MethodProt(method) => method.get_id(db),
            TyDecl::StructElement(element) => element.get_id(db),
        }
    }

    pub fn scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self {
            TyDecl::Pou(pou) => pou.scope_id(db),
            TyDecl::Variable(variable) => variable.scope_id(db),
            TyDecl::Method(method) => method.scope_id(db),
            TyDecl::MethodProt(method) => method.scope_id(db),
            TyDecl::StructElement(element) => element.scope_id(db),
        }
    }
}

// Definition of the type (POU or Spec)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum TyDef<'db> {
    Pou(PouDecl<'db>),
    Method(MethodDecl<'db>),
    MethodProt(MethodPrototype<'db>),
    Spec(Spec<'db>),
    Invalid,
}

impl<'db> TyDef<'db> {
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

    pub fn is_valid(&self) -> bool {
        !matches!(self, TyDef::Invalid)
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
            TyDef::Invalid => None,
        }
    }

    pub fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> Option<FileScopeId<'db>> {
        match self {
            TyDef::Pou(pou) => Some(pou.scope_id(db)),
            TyDef::Method(method) => Some(method.scope_id(db)),
            TyDef::MethodProt(method) => Some(method.scope_id(db)),
            TyDef::Spec(spec) => Some(spec.scope_id(db)),
            TyDef::Invalid => None,
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
    Target(Ty<'db>),
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
    Recursive,
}

#[tracing::instrument(skip_all, name = "query_type_signature")]
#[salsa::tracked]
pub fn ty_for_pou<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Ty<'db> {
    let decl = TyDecl::Pou(pou);
    let def = TyDef::Pou(pou);

    match pou.pou(db) {
        Pou::Function(func) => Ty::new(
            db,
            decl,
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

            Ty::new(db, decl, def, TyKind::FunctionBlock { extends, methods })
        }
        Pou::DataType(dt) => dt.spec(db).spec_to_ty(db, TyDecl::Pou(pou)),
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
                decl,
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
                decl,
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
    let decl = TyDecl::Method(method);
    let def = TyDef::Method(method);

    Ty::new(
        db,
        decl,
        def,
        TyKind::Method {
            is_prototype: false,
            return_type: method.return_type(db).copied(),
        },
    )
}

#[salsa::tracked]
pub fn ty_for_method_prot<'db>(db: &'db dyn BaseDatabase, method: MethodPrototype<'db>) -> Ty<'db> {
    let decl = TyDecl::MethodProt(method);
    let def = TyDef::MethodProt(method);

    Ty::new(
        db,
        decl,
        def,
        TyKind::Method {
            is_prototype: true,
            return_type: method.return_type(db).copied(),
        },
    )
}

#[salsa::tracked]
pub fn ty_for_variable<'db>(db: &'db dyn BaseDatabase, variable: VariableDecl<'db>) -> Ty<'db> {
    variable.spec(db).spec_to_ty(db, TyDecl::Variable(variable))
}

#[salsa::tracked]
pub fn ty_for_struct_field<'db>(db: &'db dyn BaseDatabase, field: StructElement<'db>) -> Ty<'db> {
    field.spec(db).spec_to_ty(db, TyDecl::StructElement(field))
}

#[salsa::tracked]
impl<'db> Ty<'db> {
    pub fn linear(
        &self,
        db: &'db dyn BaseDatabase,
        step: &PathExprWalkStep<'db>,
    ) -> Result<Ty<'db>, PathResolveError<'db>> {
        if let TyKind::Target(target) = self.kind(db) {
            return target.linear(db, step);
        }
        match &step {
            PathExprWalkStep::Field { ident, expr } => {
                let field = ident.text(db).to_string();
                match self.kind(db) {
                    // Look for a struct field
                    TyKind::Struct { elements, spec } => elements
                        .get(&ident.ident)
                        .map(|f| ty_for_struct_field(db, *f))
                        .ok_or(PathResolveError::UnknownField {
                            expr: *expr,
                            ty: *self,
                        }),
                    // Look for a method name (only in declared methods)
                    TyKind::Class { .. } | TyKind::FunctionBlock { .. } => method_table(db, *self)
                        .declared_methods
                        .get(&ident.ident)
                        .cloned()
                        .ok_or(PathResolveError::UnknownField {
                            expr: *expr,
                            ty: *self,
                        }),
                    TyKind::Array { ranges, typ } => Err(PathResolveError::NoField {
                        ty: *self,
                        expr: *expr,
                    }),
                    // Otherwise, look for a variable
                    _ => self.variables(db).get(&ident.ident).cloned().ok_or(
                        PathResolveError::UnknownField {
                            expr: *expr,
                            ty: *self,
                        },
                    ),
                }
            }
            PathExprWalkStep::Index { expr } => match self.kind(db) {
                TyKind::Array { typ, .. } => Ok(typ.spec_to_ty(db, self.decl(db))),
                _ => Err(PathResolveError::NotAnArray {
                    expr: *expr,
                    ty: *self,
                }),
            },
            PathExprWalkStep::Deref { expr, target } => match self.kind(db) {
                TyKind::RefTo(inner) => Ok(inner.spec_to_ty(db, self.decl(db))),
                _ => Err(PathResolveError::NotAReference {
                    expr: *expr,
                    ty: *self,
                }),
            },
        }
    }

    pub fn inheritors(&self, db: &'db dyn BaseDatabase) -> Vec<Ty<'db>> {
        match self.kind(db) {
            TyKind::Target(target) => target.inheritors(db),
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
    pub fn variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, Ty<'db>> {
        match self.kind(db) {
            TyKind::Target(inner) => inner.variables(db).clone(),
            _ => match self.def(db) {
                TyDef::Pou(pou) => match pou.pou(db) {
                    Pou::Function(f) => self.fetch_variables(db, &f.variables(db)),
                    Pou::FunctionBlock(fb) => self.fetch_variables(db, &fb.variables(db)),
                    _ => Default::default(),
                },
                TyDef::Method(method) => self.fetch_variables(db, &method.variables(db)),
                TyDef::MethodProt(method) => self.fetch_variables(db, &method.variables(db)),
                _ => Default::default(),
            },
        }
    }

    fn fetch_variables(
        &self,
        db: &'db dyn BaseDatabase,
        vars: &[VariableDecl<'db>],
    ) -> IndexMap<Ident, Ty<'db>> {
        let mut variables = IndexMap::default();
        for v in vars {
            let decl = TyDecl::Variable(*v);
            match v.kind(db) {
                VariableKind::Input => {
                    variables.insert(*v.name(db), v.spec(db).spec_to_ty(db, decl));
                }
                VariableKind::Output => {
                    variables.insert(*v.name(db), v.spec(db).spec_to_ty(db, decl));
                }
                VariableKind::InOut => {
                    variables.insert(*v.name(db), v.spec(db).spec_to_ty(db, decl));
                }
                _ => continue,
            };
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
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_simple(db);
        };
        matches!(self.kind(db), TyKind::Simple(_))
    }

    pub fn modifier(&self, db: &'db dyn BaseDatabase) -> Modifier {
        match self.decl(db) {
            TyDecl::Method(m) => m.modifier(db),
            TyDecl::Pou(f) => f.modifier(db),
            _ => Modifier::default(),
        }
    }

    pub fn is_callable(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_callable(db);
        };
        matches!(
            self.kind(db),
            TyKind::Function { .. } | TyKind::FunctionBlock { .. } | TyKind::Method { .. }
        )
    }

    pub fn is_target(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::Target(_))
    }

    pub fn is_method_prototype(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_method_prototype(db);
        };
        if let TyKind::Method { is_prototype, .. } = self.kind(db) {
            return *is_prototype;
        }
        false
    }

    pub fn is_unresolved(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_unresolved(db);
        };
        matches!(self.kind(db), TyKind::Unresolved(_))
    }

    pub fn is_recursive(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.is_recursive(db);
        };
        matches!(self.kind(db), TyKind::Recursive)
    }

    pub fn is_reference(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), TyKind::RefTo(_))
    }

    pub fn is_variable(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.decl(db), TyDecl::Variable(_))
    }

    pub fn is_variable_input(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyDecl::Variable(var) = self.decl(db) {
            return var.kind(db) == VariableKind::Input;
        }
        false
    }

    pub fn is_variable_inout(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyDecl::Variable(var) = self.decl(db) {
            return var.kind(db) == VariableKind::InOut;
        }
        false
    }

    pub fn is_variable_output(&self, db: &'db dyn BaseDatabase) -> bool {
        if let TyDecl::Variable(var) = self.decl(db) {
            return var.kind(db) == VariableKind::Output;
        }
        false
    }

    pub fn has_return_type(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>> {
        if let TyKind::Target(sig) = self.kind(db) {
            return sig.has_return_type(db);
        };
        match self.kind(db) {
            TyKind::Function { return_type } => {
                return_type.map(|rt| rt.spec_to_ty(db, self.decl(db)))
            }
            TyKind::Method { return_type, .. } => {
                return_type.map(|rt| rt.spec_to_ty(db, self.decl(db)))
            }
            _ => None,
        }
    }
}

#[salsa::tracked]
impl<'db> Spec<'db> {
    #[salsa::tracked]
    pub fn spec_to_ty_kind(self, db: &'db dyn BaseDatabase) -> TyKind<'db> {
        match self.kind(db) {
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
                    Some(pou) => {
                        let ty = ty_for_pou(db, pou);
                        TyKind::Target(ty)
                    }
                    None => TyKind::Unresolved(*target),
                }
            }
            SpecKind::Simple(simple) => TyKind::Simple(*simple),
            SpecKind::ArrayConformand(array) => TyKind::ArrayConformand { typ: *array },
            SpecKind::Ref(_ref) => TyKind::RefTo(*_ref),
        }
    }

    #[salsa::tracked]
    pub fn spec_to_ty(self, db: &'db dyn BaseDatabase, origin: TyDecl<'db>) -> Ty<'db> {
        let kind = self.spec_to_ty_kind(db);
        let def = match kind {
            TyKind::Unresolved(_) => TyDef::Invalid,
            TyKind::Target(target) => target.def(db),
            _ => TyDef::Spec(self),
        };
        Ty::new(db, origin, def, kind)
    }
}

impl<'db> TypeInfo<'db> for Ty<'db> {
    fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            TyKind::Simple(elem) => elem.type_name(db),
            TyKind::Target(t) => t.type_name(db),
            TyKind::Enum { .. } => "ENUM".into(),
            TyKind::SubRange { .. } => "SUBRANGE".into(),
            TyKind::RefTo(ref_) => format!(
                "REF_TO {}",
                ref_.spec_to_ty(db, self.decl(db)).type_name(db)
            ),
            TyKind::Array { .. } => "ARRAY".into(),
            TyKind::ArrayConformand { .. } => "ARRAY*".into(),
            TyKind::Struct { .. } => "STRUCT".into(),
            TyKind::Interface { .. } => "INTERFACE".into(),
            TyKind::Class { .. } => "CLASS".into(),
            TyKind::Function { .. } => "FUNCTION".into(),
            TyKind::FunctionBlock { .. } => "FUNCTION_BLOCK".into(),
            TyKind::Method { .. } => "METHOD".into(),
            TyKind::Unresolved(_) => "{unknown}".into(),
            TyKind::Recursive => "{recursive}".into(),
        }
    }

    fn decl_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self.decl(db) {
            TyDecl::Pou(pou) => match pou.pou(db) {
                Pou::Function(_) => "FUNCTION".to_string(),
                Pou::FunctionBlock(_) => "FUNCTION_BLOCK".to_string(),
                Pou::DataType(dt) => match dt.spec(db).kind(db) {
                    SpecKind::Array(_) => "ARRAY".to_string(),
                    SpecKind::Enum { .. } => "ENUM".to_string(),
                    SpecKind::Subrange { .. } => "SUBRANGE".to_string(),
                    SpecKind::Struct { .. } => "STRUCT".to_string(),
                    SpecKind::Target(_) => "TYPE".to_string(),
                    SpecKind::Simple(_) => "TYPE".to_string(),
                    SpecKind::ArrayConformand { .. } => "ARRAY*".to_string(),
                    SpecKind::Ref { .. } => "REF_TO".to_string(),
                },
                Pou::Class(_) => "CLASS".to_string(),
                Pou::Interface(_) => "INTERFACE".to_string(),
            },
            TyDecl::Variable(var) => match var.kind(db) {
                VariableKind::Input => "VAR_INPUT".to_string(),
                VariableKind::Output => "VAR_OUTPUT".to_string(),
                VariableKind::InOut => "VAR_IN_OUT".to_string(),
                VariableKind::Var => "VAR".to_string(),
                VariableKind::Temp => "VAR_TEMP".to_string(),
                VariableKind::Global => "VAR_GLOBAL".to_string(),
                VariableKind::External => "VAR_EXTERNAL".to_string(),
                VariableKind::Access => "VAR_ACCESS".to_string(),
                VariableKind::Config => "VAR_CONFIG".to_string(),
            },
            TyDecl::Method(m) => "METHOD".to_string(),
            TyDecl::MethodProt(m) => "METHOD (prototype)".to_string(),
            TyDecl::StructElement(e) => "STRUCT field".to_string(),
        }
    }
}
