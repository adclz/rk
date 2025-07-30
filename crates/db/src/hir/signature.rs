use std::iter::FusedIterator;

use auto_lsp::default::db::BaseDatabase;

use crate::hir::{
    expressions::spec::{CompositeSpecKind, Enum, Spec, SpecKind},
    interned::{identifier::Ident, namespace::NamespaceAccess},
    pous::{
        pou::{Pou, PouDecl},
        variable::VariableKind,
    },
};

#[salsa::tracked(returns(ref))]
pub fn signature_for_pou<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Signature<'db> {
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
        }
        Pou::FunctionBlock(fb) => fb
            .variables(db)
            .iter()
            .filter_map(|v| match v.kind(db) {
                VariableKind::Input | VariableKind::Output | VariableKind::InOut => Some(v),
                _ => None,
            })
            .for_each(|v| parameters.push(v.spec(db).to_parameter(db))),
        Pou::DataType(dt) => {
            parameters.push(dt.spec(db).to_parameter(db));
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
        }
        _ => todo!(),
    }

    Signature {
        specs: parameters,
        return_type,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct Signature<'db> {
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
    Simple(Spec<'db>),
    // enums can have an integer or one of the enum values
    NumberOrOneOfList {
        spec: Spec<'db>,
        list: Vec<Ident>,
    },
    ArrayLike {
        spec: Spec<'db>,
    },
    SubRange,
    Composite {
        kind: CompositeKind,
        elements: Vec<Parameter<'db>>,
    },
    Target(NamespaceAccess),
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum CompositeKind {
    Struct,
    FunctionBlock,
}

impl<'db> Spec<'db> {
    pub fn to_parameter(&self, db: &'db dyn BaseDatabase) -> Parameter<'db> {
        match self.kind(db) {
            SpecKind::Simple(simple) => Parameter {
                name: None,
                kind: ParameterKind::Simple(*self),
            },

            SpecKind::Composite(CompositeSpecKind::Array(array)) => Parameter {
                name: None,
                kind: ParameterKind::ArrayLike {
                    spec: *array.of_type,
                },
            },

            SpecKind::Composite(CompositeSpecKind::Enum(enum_)) => match enum_ {
                Enum::Anonymous(list) => Parameter {
                    name: None,
                    kind: ParameterKind::NumberOrOneOfList {
                        spec: *self,
                        list: list.iter().map(|name| *name).collect(),
                    },
                },
                Enum::Named(list) => Parameter {
                    name: None,
                    kind: ParameterKind::NumberOrOneOfList {
                        spec: *self,
                        list: list.iter().map(|(name, _)| *name).collect(),
                    },
                },
            },

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
                    kind: ParameterKind::Composite {
                        kind: CompositeKind::Struct,
                        elements,
                    },
                }
            }

            SpecKind::Composite(CompositeSpecKind::Subrange(subrange)) => Parameter {
                name: None,
                kind: ParameterKind::SubRange,
            },

            SpecKind::Target(target) => Parameter {
                name: None,
                kind: ParameterKind::Target(*target),
            },
        }
    }
}

pub struct ParameterIterator<'db> {
    db: &'db dyn BaseDatabase,
    signature: &'db Signature<'db>,

    nested_iter: Option<std::slice::Iter<'db, Parameter<'db>>>,
}

impl<'db> Signature<'db> {
    pub fn iter(&'db self, db: &'db dyn BaseDatabase) -> ParameterIterator<'db> {
        ParameterIterator {
            db,
            signature: self,
            nested_iter: None,
        }
    }
}

impl<'db> Iterator for ParameterIterator<'db> {
    type Item = &'db Parameter<'db>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(iter) = &mut self.nested_iter {
            if let Some(param) = iter.next() {
                return Some(param);
            } else {
                self.nested_iter = None;
            }
        }

        match self.signature.specs.iter().next() {
            Some(param) => match &param.kind {
                ParameterKind::Composite { kind: _, elements } => {
                    self.nested_iter = Some(elements.iter());
                    Some(param)
                }
                _ => Some(param),
            },
            None => None,
        }
    }
}

impl FusedIterator for ParameterIterator<'_> {}
