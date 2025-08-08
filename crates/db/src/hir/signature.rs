use std::sync::Arc;

use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::hir::{
    expressions::spec::{Enum, Spec, SpecKind},
    interned::{
        identifier::Ident,
        namespace::{NamespaceAccess, SpannedNamespaceAccess},
    },
    pous::{
        pou::{Pou, PouDecl},
        variable::VariableKind,
    },
    scopes::{
        scope::{ScopeId},
        solver::resolve_namespace_access,
    },
    semantic_index::{SemanticIndex},
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct TypeSignature<'db> {
    pub name: Option<Ident>,
    pub kind: SignatureKind<'db>,
    pub specs: FxHashMap<Ident, TypeParameter<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SignatureKind<'db> {
    Pou(PouDecl<'db>),
    // Errored variants
    Recursive(PouDecl<'db>),
    Never(SpannedNamespaceAccess),
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct CallableSignature<'db> {
    pub kind: CallableSignatureKind<'db>,
    pub input_section: FxHashMap<Ident, TypeParameter<'db>>,
    pub output_section: FxHashMap<Ident, TypeParameter<'db>>,
    pub in_out_section: FxHashMap<Ident, TypeParameter<'db>>,
    pub return_type: Option<TypeParameter<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum CallableSignatureKind<'db> {
    Function(PouDecl<'db>),
    FunctionBlock(PouDecl<'db>),
    Method(PouDecl<'db>),
    // Errored variants
    Recursive(PouDecl<'db>),
    Never(SpannedNamespaceAccess),
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum TypeParameter<'db> {
    // Single type spec (usually a literal)
    Simple(Spec<'db>),
    // enums can have an integer or one of the enum values
    Enum {
        spec: Spec<'db>,
        list: Vec<Ident>,
    },
    Array {
        spec: Spec<'db>,
    },
    SubRange,
    Struct(FxHashMap<Ident, TypeParameter<'db>>),
    RefTo(Arc<TypeParameter<'db>>),
    Pou(Arc<TypeSignature<'db>>),
    // Errors
    Unresolved(NamespaceAccess),
    // usually means a function is being used
    Incompatible(PouDecl<'db>),
}

fn type_signature_result<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> Option<Arc<TypeSignature<'db>>> {
    TypeSignature::recursive(db, pou)
}

#[tracing::instrument(skip_all, name = "query_type_signature")]
#[salsa::tracked(cycle_result = type_signature_result)]
pub fn type_signature<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> Option<Arc<TypeSignature<'db>>> {
    let kind = SignatureKind::Pou(pou);
    let mut parameters = FxHashMap::default();

    match pou.pou(db) {
        Pou::FunctionBlock(fb) => {
            for v in fb.variables(db) {
                if matches!(
                    v.kind(db),
                    VariableKind::Input | VariableKind::Output | VariableKind::InOut
                ) {
                    parameters.insert(*v.name(db), v.spec(db).to_type_parameter(db));
                }
            }
        }
        Pou::DataType(dt) => {
            parameters.insert(*pou.name(db), dt.spec(db).to_type_parameter(db));
        }
        Pou::Class(class) => {
            for v in class.variables(db) {
                if matches!(
                    v.kind(db),
                    VariableKind::Input | VariableKind::Output | VariableKind::InOut
                ) {
                    parameters.insert(*v.name(db), v.spec(db).to_type_parameter(db));
                }
            }
        }
        // Interfaces and Functions do not have variables
        _ => return None,
    }

    Some(Arc::new(TypeSignature {
        kind,
        name: Some(*pou.name(db)),
        specs: parameters,
    }))
}

fn call_signature_result<'db>(
    db: &'db dyn BaseDatabase,
    value: PouDecl<'db>,
) -> Option<Arc<CallableSignature<'db>>> {
    CallableSignature::recursive(db, value)
}

impl<'db> CallableSignature<'db> {
    pub fn recursive(
        db: &'db dyn BaseDatabase,
        pou: PouDecl<'db>,
    ) -> Option<Arc<CallableSignature<'db>>> {
        Some(Arc::new(CallableSignature {
            kind: CallableSignatureKind::Recursive(pou),
            input_section: FxHashMap::default(),
            output_section: FxHashMap::default(),
            in_out_section: FxHashMap::default(),
            return_type: None,
        }))
    }
}

#[tracing::instrument(skip_all, name = "query_call_signature")]
#[salsa::tracked(cycle_result = call_signature_result)]
pub fn call_signature<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> Option<Arc<CallableSignature<'db>>> {
    let kind = match pou.pou(db) {
        Pou::Function(f) => CallableSignatureKind::Function(pou),
        Pou::FunctionBlock(fb) => CallableSignatureKind::FunctionBlock(pou),
        Pou::Class(c) => CallableSignatureKind::Method(pou),
        Pou::DataType(_) | Pou::Interface(_) => return None,
    };

    let mut input_section = FxHashMap::default();
    let mut output_section = FxHashMap::default();
    let mut in_out_section = FxHashMap::default();
    let mut return_type = None;

    match pou.pou(db) {
        Pou::Function(f) => {
            for v in f.variables(db) {
                match v.kind(db) {
                    VariableKind::Input => {
                        input_section.insert(*v.name(db), v.spec(db).to_type_parameter(db))
                    }
                    VariableKind::Output => {
                        output_section.insert(*v.name(db), v.spec(db).to_type_parameter(db))
                    }
                    VariableKind::InOut => {
                        in_out_section.insert(*v.name(db), v.spec(db).to_type_parameter(db))
                    }
                    _ => continue,
                };
            }
            return_type = f.return_type(db).map(|s| s.to_type_parameter(db));
        }
        Pou::FunctionBlock(fb) => {
            for v in fb.variables(db) {
                match v.kind(db) {
                    VariableKind::Input => {
                        input_section.insert(*v.name(db), v.spec(db).to_type_parameter(db))
                    }
                    VariableKind::Output => {
                        output_section.insert(*v.name(db), v.spec(db).to_type_parameter(db))
                    }
                    VariableKind::InOut => {
                        in_out_section.insert(*v.name(db), v.spec(db).to_type_parameter(db))
                    }
                    _ => continue,
                };
            }
        }
        Pou::Class(c) => {
            for v in c.variables(db) {
                match v.kind(db) {
                    VariableKind::Input => {
                        input_section.insert(*v.name(db), v.spec(db).to_type_parameter(db))
                    }
                    VariableKind::Output => {
                        output_section.insert(*v.name(db), v.spec(db).to_type_parameter(db))
                    }
                    VariableKind::InOut => {
                        in_out_section.insert(*v.name(db), v.spec(db).to_type_parameter(db))
                    }
                    _ => continue,
                };
            }
        }
        Pou::DataType(_) | Pou::Interface(_) => return None,
    }
    Some(Arc::new(CallableSignature {
        kind,
        input_section,
        output_section,
        in_out_section,
        return_type,
    }))
}

impl<'db> TypeSignature<'db> {
    pub fn recursive(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Option<Arc<TypeSignature<'db>>> {
        Some(Arc::new(TypeSignature {
            name: Some(*pou.name(db)),
            kind: SignatureKind::Recursive(pou),
            specs: FxHashMap::default(),
        }))
    }

    pub fn signature_to_string(
        &self,
        db: &'db dyn BaseDatabase,
        sema: &SemanticIndex<'db>,
        scope: ScopeId,
    ) -> String {
        let mut result = String::new();

        let mut infinite_size = false;

        let pou = match &self.kind {
            SignatureKind::Pou(pou) => pou,
            SignatureKind::Recursive(pou) => {
                infinite_size = true;
                pou
            }
            SignatureKind::Never(target) => {
                return "{unknown}".to_string();
            }
        };

        let (head, end) = match pou.pou(db) {
            Pou::Function(_) => ("FUNCTION", "END_FUNCTION"),
            Pou::FunctionBlock(_) => ("FUNCTION_BLOCK", "END_FUNCTION_BLOCK"),
            Pou::DataType(_) => ("TYPE", "END_TYPE"),
            Pou::Interface(_) => ("INTERFACE", "END_INTERFACE"),
            Pou::Class(_) => ("CLASS", "END_CLASS"),
        };

        let name = match &self.name {
            Some(name) => name.text(db),
            None => "{unknown}",
        };

        let infinite = if infinite_size {
            "\n(infinite size !)\n"
        } else {
            ""
        };

        result.push_str(
            format!(
                r#"{head} {name}
{infinite}
{end}
"#
            )
            .as_str(),
        );

        result
    }
}

impl<'db> Spec<'db> {
    pub fn to_type_parameter(&self, db: &'db dyn BaseDatabase) -> TypeParameter<'db> {
        match self.kind(db) {
            SpecKind::Array(array) => TypeParameter::Array {
                spec: *array.of_type,
            },
            SpecKind::Enum(enum_) => {
                match enum_ {
                    Enum::Anonymous(list) => TypeParameter::Enum {
                        spec: *self,
                        list: list.to_vec(),
                    },
                    Enum::Named(list) => TypeParameter::Enum {
                        spec: *self,
                        list: list.iter().map(|(name, _)| *name).collect(),
                    },
                }
            }

            SpecKind::Struct(struct_) => TypeParameter::Struct(
                struct_
                    .elements
                    .iter()
                    .map(|e| (e.name, e.spec.to_type_parameter(db)))
                    .collect::<FxHashMap<Ident, TypeParameter<'db>>>(),
            ),

            SpecKind::Subrange(subrange) => TypeParameter::SubRange,

            SpecKind::Target(target) => {
                match resolve_namespace_access(db, self.file(db), self.scope_id(db), *target) {
                    Some(pou) => {
                        let signature = type_signature(db, pou);
                        match signature {
                            Some(signature) => TypeParameter::Pou(signature),
                            None => TypeParameter::Incompatible(pou),
                        }
                    }
                    None => TypeParameter::Unresolved(*target),
                }
            }
            SpecKind::Ref(target) => TypeParameter::RefTo(target.to_type_parameter(db).into()),
            _  => TypeParameter::Simple(*self),
        }
    }
}

pub struct CallableSignatureIter<'db> {
    pub(crate) signature: &'db CallableSignature<'db>,
    pub(crate) db: &'db dyn BaseDatabase,
    pub(crate) input_iter: std::collections::hash_map::Iter<'db, Ident, TypeParameter<'db>>,
    pub(crate) output_iter: std::collections::hash_map::Iter<'db, Ident, TypeParameter<'db>>,
    pub(crate) in_out_iter: std::collections::hash_map::Iter<'db, Ident, TypeParameter<'db>>,
    pub(crate) current_section: Option<VariableKind>,
}

impl<'db> Iterator for CallableSignatureIter<'db> {
    type Item = (&'db Ident, &'db TypeParameter<'db>, VariableKind);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.current_section {
                Some(VariableKind::Input) => {
                    if let Some(item) = self.input_iter.next() {
                        return Some((item.0, item.1, VariableKind::Input));
                    } else {
                        self.current_section = Some(VariableKind::Output);
                    }
                }
                Some(VariableKind::Output) => {
                    if let Some(item) = self.output_iter.next() {
                        return Some((item.0, item.1, VariableKind::Output));
                    } else {
                        self.current_section = Some(VariableKind::InOut);
                    }
                }
                Some(VariableKind::InOut) => {
                    if let Some(item) = self.in_out_iter.next() {
                        return Some((item.0, item.1, VariableKind::InOut));
                    } else {
                        self.current_section = None;
                    }
                }
                _ => return None,
            }
        }
    }
}
