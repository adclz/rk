use std::{fmt::format, iter::FusedIterator, sync::Arc};

use auto_lsp::default::db::BaseDatabase;

use crate::hir::{
    expressions::spec::{CompositeSpecKind, Enum, SimpleSpecKind, Spec, SpecKind},
    interned::{
        identifier::Ident,
        namespace::{NamespaceAccess, SpannedNamespaceAccess},
    },
    pous::{
        pou::{Pou, PouDecl},
        variable::VariableKind,
    },
    scopes::{
        scope::{ScopeId, FilePouId},
        solver::resolve_namespace_access,
    },
    semantic_index::{semantic_index, SemanticIndex},
};

fn signature_result<'db>(db: &'db dyn BaseDatabase, value: PouDecl<'db>) -> Arc<Signature<'db>> {
    Signature::recursive(db, value.pou_id(db))
}

#[salsa::tracked(cycle_result = signature_result)]
pub fn signature_for_pou<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Arc<Signature<'db>> {
    let kind = SignatureKind::Pou(pou.pou_id(db));
    let mut return_type = None;
    let mut parameters = vec![];

    match pou.pou(db) {
        Pou::Function(f) => {
            f.variables(db)
                .iter()
                .filter_map(|v| match v.kind(db) {
                    VariableKind::Input | VariableKind::Output | VariableKind::InOut => Some(v),
                    _ => None,
                })
                .for_each(|v| parameters.push(v.spec(db).to_parameter(db, Some(*v.name(db)))));
            return_type = f.return_type(db).copied();
        }
        Pou::FunctionBlock(fb) => {
            fb.variables(db)
                .iter()
                .filter_map(|v| match v.kind(db) {
                    VariableKind::Input | VariableKind::Output | VariableKind::InOut => Some(v),
                    _ => None,
                })
                .for_each(|v| parameters.push(v.spec(db).to_parameter(db, Some(*v.name(db)))));
        }
        Pou::DataType(dt) => {
            parameters.push(dt.spec(db).to_parameter(db, None));
        }
        Pou::Interface(interface) => {
            interface.methods(db).iter().for_each(|method| {
                method
                    .variables(db)
                    .iter()
                    .filter_map(|v| match v.kind(db) {
                        VariableKind::Input | VariableKind::Output | VariableKind::InOut => Some(v),
                        _ => None,
                    })
                    .for_each(|v| parameters.push(v.spec(db).to_parameter(db, None)));
                return_type = method.return_type(db).copied();
            });
        }
        Pou::Class(class) => {
            todo!()
        }
    }

    Arc::new(Signature {
        kind,
        name: Some(*pou.name(db)),
        specs: parameters,
        return_type,
    })
}

impl<'db> Signature<'db> {
    pub fn recursive(db: &'db dyn BaseDatabase, pou: FilePouId) -> Arc<Signature<'db>> {
        let sema = semantic_index(db, pou.1);
        let id = sema.pou_keys[&pou.0];
        Arc::new(Signature {
            name: Some(*id.name(db)),
            kind: SignatureKind::Recursive(pou),
            specs: vec![],
            return_type: None,
        })
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

        let sema = semantic_index(db, pou.1);
        let pou = sema.pou_keys[&pou.0];

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

        let parameters = self
            .iter(db)
            .map(|param| {
                let name = param
                    .name
                    .map(|i| i.text(db).to_string())
                    .unwrap_or_else(|| "{unknown}".to_string());
                format!(
r#"    {}: {}"#,
                    name,
                    match &param.kind {
                        ParameterKind::Simple(simple) => simple.to_string().to_string(),
                        ParameterKind::Enum { spec, list } => {
                            format!("(enum) ({} variants)", list.len()).to_string()
                        },
                        ParameterKind::Array { spec } => format!("[]{}", spec.to_string(db, &sema)).to_string(),
                        ParameterKind::SubRange => "(subrange)".to_string(),
                        ParameterKind::Struct(fields) => {
                            format!("(struct)").to_string()
                        }   
                        ParameterKind::Pou(sig) => {
                            format!("POU").to_string()
                        }
                        ParameterKind::Unresolved(ns) => "{unknown}".to_string(),
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let infinite = if infinite_size {
            "(infinite size !)\n"
        } else {
            ""
        };

        result.push_str(
            format!(
                r#"{infinite}{head} {name}
{parameters}
{end}
"#
            )
            .as_str(),
        );

        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SignatureKind {
    Pou(FilePouId),
    Recursive(FilePouId),
    Never(SpannedNamespaceAccess),
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct Signature<'db> {
    pub name: Option<Ident>,
    pub kind: SignatureKind,
    pub specs: Vec<Parameter<'db>>,
    pub return_type: Option<Spec<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct Parameter<'db> {
    pub name: Option<Ident>,
    pub kind: ParameterKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ParameterKind<'db> {
    // Single type spec (usually a literal)
    Simple(SimpleSpecKind),
    // enums can have an integer or one of the enum values
    Enum {
        spec: SimpleSpecKind,
        list: Vec<Ident>,
    },
    Array {
        spec: Spec<'db>,
    },
    SubRange,
    // A struct is not a POU, but a complex type
    Struct(Vec<Parameter<'db>>),
    Pou(Arc<Signature<'db>>),
    Unresolved(NamespaceAccess),
}

impl<'db> Spec<'db> {
    pub fn to_parameter(&self, db: &'db dyn BaseDatabase, name: Option<Ident>) -> Parameter<'db> {
        match self.kind(db) {
            SpecKind::Simple(simple) => Parameter {
                name,
                kind: ParameterKind::Simple(*simple),
            },

            SpecKind::Composite(CompositeSpecKind::Array(array)) => Parameter {
                name,
                kind: ParameterKind::Array {
                    spec: *array.of_type,
                },
            },

            SpecKind::Composite(CompositeSpecKind::Enum(enum_)) => {
                let kind = SimpleSpecKind::UInt;
                match enum_ {
                    Enum::Anonymous(list) => Parameter {
                        name,
                        kind: ParameterKind::Enum {
                            spec: kind,
                            list: list.iter().map(|name| *name).collect(),
                        },
                    },
                    Enum::Named(list) => Parameter {
                        name,
                        kind: ParameterKind::Enum {
                            spec: kind,
                            list: list.iter().map(|(name, _)| *name).collect(),
                        },
                    },
                }
            }

            SpecKind::Composite(CompositeSpecKind::Struct(struct_)) => Parameter {
                name,
                kind: ParameterKind::Struct(
                    struct_
                        .elements
                        .iter()
                        .map(|e| Parameter {
                            name: Some(e.name),
                            kind: e.spec.to_parameter(db, None).kind,
                        })
                        .collect(),
                ),
            },

            SpecKind::Composite(CompositeSpecKind::Subrange(subrange)) => Parameter {
                name,
                kind: ParameterKind::SubRange,
            },

            SpecKind::Target(target) => {
                match resolve_namespace_access(db, self.file(db), self.scope_id(db), *target) {
                    Some(id) => {
                        let sema = semantic_index(db, self.file(db));
                        let pou = sema.pou_keys[&id.0];

                        let signature = signature_for_pou(db, pou);

                        Parameter {
                            name: None,
                            kind: ParameterKind::Pou(signature),
                        }
                    }
                    None => Parameter {
                        name,
                        kind: ParameterKind::Unresolved(*target),
                    },
                }
            }
        }
    }
}

pub struct ParameterIterator<'db> {
    _db: &'db dyn BaseDatabase,

    outer_iter: std::slice::Iter<'db, Parameter<'db>>,
    nested_iter: Option<std::slice::Iter<'db, Parameter<'db>>>,
}

impl<'db> Signature<'db> {
    pub fn iter(&'db self, db: &'db dyn BaseDatabase) -> ParameterIterator<'db> {
        ParameterIterator {
            _db: db,
            nested_iter: None,
            outer_iter: self.specs.iter(),
        }
    }
}

impl<'db> Iterator for ParameterIterator<'db> {
    type Item = &'db Parameter<'db>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(nested) = &mut self.nested_iter {
                if let Some(param) = nested.next() {
                    return Some(param);
                }
                self.nested_iter = None;
            }

            if let Some(param) = self.outer_iter.next() {
                match &param.kind {
                    ParameterKind::Pou(sig) => {
                        self.nested_iter = Some(sig.specs.iter());
                    }
                    _ => return Some(param),
                }
            } else {
                return None;
            }
        }
    }
}

impl FusedIterator for ParameterIterator<'_> {}
