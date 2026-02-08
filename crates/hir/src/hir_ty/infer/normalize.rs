use db::WorkspaceDataBase;

use crate::{
    HirNodeInfo,
    hir_def::{
        expressions::{expression::MultibitsPart, spec::ElementarySpec},
        pous::variable::DirectVariable,
    },
    hir_ty::{
        head::signature::infer_signature,
        ty::{CallableType, Type},
    },
};

/*
    Normalizes a type into its identity data type.
    This is an eager, lossy operation: information about how a value was
    reached (variable access, path steps, call origin, etc.) is discarded.
*/

impl<'db> Type<'db> {
    pub fn normalize(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        match self {
            Type::DataType(dt) => {
                infer_signature(db, dt.get_scope_id(db)).type_of_specs[&dt.spec(db)].normalize(db)
            }
            Type::Variable((var, multibits)) => {
                if let Some(multibits) = multibits {
                    return multibits_to_type(db, *multibits);
                }
                infer_signature(db, var.get_scope_id(db)).type_of_specs[&var.spec(db)].normalize(db)
            }
            Type::CallableType(typ) => match typ {
                CallableType::Function(f) => match f.return_type(db) {
                    Some(ret_ty) => {
                        infer_signature(db, f.get_scope_id(db)).type_of_specs[ret_ty].normalize(db)
                    }
                    _ => Type::Void,
                },
                CallableType::MethodDecl(m) => match m.return_type(db) {
                    Some(ret_ty) => {
                        infer_signature(db, m.get_scope_id(db)).type_of_specs[ret_ty].normalize(db)
                    }
                    _ => Type::Void,
                },
                _ => *self,
            },
            Type::DirectVariable((dv, multibits)) => {
                if let Some(multibits) = multibits {
                    return multibits_to_type(db, *multibits);
                }
                direct_variable_to_type(db, *dv, *multibits)
            }
            Type::StructElement(element) => infer_signature(db, element.get_scope_id(db))
                .type_of_specs[&element.spec(db)]
                .normalize(db),
            _ => *self,
        }
    }
}

// Table 16 – Directly represented variables
pub fn direct_variable_to_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    dv: DirectVariable,
    multibits: Option<MultibitsPart>,
) -> Type<'db> {
    let chars = dv.adress(db).text(db).chars();
    // XBWDL
    match dv.adress(db).text(db).chars().nth(1) {
        // BIT
        Some('X') => Type::new_bool(),
        // BYTE
        Some('B') => Type::Elementary(ElementarySpec::Byte),
        // WORD
        Some('W') => Type::Elementary(ElementarySpec::Word),
        // DWORD
        Some('D') => Type::Elementary(ElementarySpec::DWord),
        // LWORD
        Some('L') => Type::Elementary(ElementarySpec::LWord),
        _ => {
            /* err: invalid location type */
            Type::Never
        }
    }
}

// Table 17 – Partial access of ANY_BIT variables
pub fn multibits_to_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    multibits: MultibitsPart,
) -> Type<'db> {
    match multibits {
        // an offset just returns a BOOL
        MultibitsPart::Offset(offset) => Type::new_bool(),
        MultibitsPart::AccessOffset { access, offset } => {
            let mut chars = access.text(db).chars();
            match chars.next() {
                // BIT
                Some('X') => Type::new_bool(),
                // BYTE
                Some('B') => Type::Elementary(ElementarySpec::Byte),
                // WORD
                Some('W') => Type::Elementary(ElementarySpec::Word),
                // DWORD
                Some('D') => Type::Elementary(ElementarySpec::DWord),
                // LWORD
                Some('L') => Type::Elementary(ElementarySpec::LWord),
                _ => {
                    // No %X assumes Bool
                    // although this might also be a parse error
                    Type::new_bool()
                }
            }
        }
    }
}
