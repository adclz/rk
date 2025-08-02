use std::iter::FusedIterator;

use auto_lsp::default::db::{BaseDatabase};

use crate::hir::{
    expressions::spec::{CompositeSpecKind, Enum, SimpleSpecKind, Spec, SpecKind},
    interned::{identifier::Ident, namespace::NamespaceAccess},
    pous::{
        pou::{Pou, PouDecl},
        variable::VariableKind,
    },
};

#[salsa::tracked(returns(ref))]
pub fn signature_for_pou<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> Signature<'db> {
    let kind;
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
                .for_each(|v| parameters.push(v.spec(db).to_parameter(db)));
            return_type = f.return_type(db).copied();
            kind = SignatureKind::Function;
        }
        Pou::FunctionBlock(fb) => {
            fb.variables(db)
                .iter()
                .filter_map(|v| match v.kind(db) {
                    VariableKind::Input | VariableKind::Output | VariableKind::InOut => Some(v),
                    _ => None,
                })
                .for_each(|v| parameters.push(v.spec(db).to_parameter(db)));
            kind = SignatureKind::FunctionBlock;
        }
        Pou::DataType(dt) => {
            parameters.push(dt.spec(db).to_parameter(db));
            kind = SignatureKind::DataType;
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
                    .for_each(|v| parameters.push(v.spec(db).to_parameter(db)));
                return_type = method.return_type(db).copied();
            });
            kind = SignatureKind::Interface;
        }
        Pou::Class(class) => {
            todo!()
        }
    }

    Signature {
        kind,
        specs: parameters,
        return_type,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SignatureKind {
    Function,
    FunctionBlock,
    DataType,
    Interface,
    Struct,
    Method,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct Signature<'db> {
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
    Composite(Signature<'db>),
    // Will be solved later when using "resolve_spec"
    Unresolved(NamespaceAccess)
}

impl<'db> Spec<'db> {
    pub fn to_parameter(&self, db: &'db dyn BaseDatabase) -> Parameter<'db> {
        match self.kind(db) {
            SpecKind::Simple(simple) => Parameter {
                name: None,
                kind: ParameterKind::Simple(*simple),
            },

            SpecKind::Composite(CompositeSpecKind::Array(array)) => Parameter {
                name: None,
                kind: ParameterKind::Array {
                    spec: *array.of_type,
                },
            },

            SpecKind::Composite(CompositeSpecKind::Enum(enum_)) => {
                let kind = SimpleSpecKind::UInt;
                match enum_ {
                    Enum::Anonymous(list) => Parameter {
                        name: None,
                        kind: ParameterKind::Enum {
                            spec: kind,
                            list: list.iter().map(|name| *name).collect(),
                        },
                    },
                    Enum::Named(list) => Parameter {
                        name: None,
                        kind: ParameterKind::Enum {
                            spec: kind,
                            list: list.iter().map(|(name, _)| *name).collect(),
                        },
                    },
                }
            }

            SpecKind::Composite(CompositeSpecKind::Struct(struct_)) => {
                let elements = struct_
                    .elements
                    .iter()
                    .map(|e| Parameter {
                        name: Some(e.name),
                        kind: e.spec.to_parameter(db).kind,
                    })
                    .collect();
                Parameter {
                    name: None,
                    kind: ParameterKind::Composite(Signature {
                        kind: SignatureKind::Struct,
                        specs: elements,
                        return_type: None,
                    }),
                }
            }

            SpecKind::Composite(CompositeSpecKind::Subrange(subrange)) => Parameter {
                name: None,
                kind: ParameterKind::SubRange,
            },

            SpecKind::Target(target) => Parameter {
                name: None,
                kind: ParameterKind::Unresolved(*target),
            },
        }
    }
}

pub struct ParameterIterator<'db> {
    db: &'db dyn BaseDatabase,

    outer_iter: std::slice::Iter<'db, Parameter<'db>>,
    nested_iter: Option<std::slice::Iter<'db, Parameter<'db>>>,
}

impl<'db> Signature<'db> {
    pub fn iter(&'db self, db: &'db dyn BaseDatabase) -> ParameterIterator<'db> {
        ParameterIterator {
            db,
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
                    ParameterKind::Composite(sig) => {
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
