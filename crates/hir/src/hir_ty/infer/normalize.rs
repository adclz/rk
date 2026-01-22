use db::WorkspaceDataBase;

use crate::{
    hir_def::{
        expressions::{expression::MultibitsPart, spec::ElementarySpec},
        pous::variable::DirectVariable,
    },
    hir_ty::ty::{CallableType, Type},
};

/*
    Normalizes a type into its identity data type.
    This is an eager, lossy operation: information about how a value was
    reached (variable access, path steps, call origin, etc.) is discarded.
*/

impl<'db> Type<'db> {
    pub fn normalize(&self, db: &'db dyn WorkspaceDataBase) -> Type<'db> {
        match self {
            Type::DataType(dt) => Type::new_spec(db, dt.spec(db)),
            Type::Variable((var, multibits)) => {
                let var_typ = Type::new_spec(db, var.spec(db)).normalize(db);
                if let Some(multibits) = multibits {
                    return multibits_to_type(db, *multibits);
                }
                var_typ
            }
            Type::CallableType(typ) => match typ {
                CallableType::Function(f) => match f.return_type(db) {
                    Some(ret_ty) => Type::new_spec(db, *ret_ty).normalize(db),
                    _ => Type::Void,
                },
                CallableType::MethodDecl(m) => match m.return_type(db) {
                    Some(ret_ty) => Type::new_spec(db, *ret_ty).normalize(db),
                    _ => Type::Void,
                },
                _ => *self,
            },
            Type::DirectVariable((dv, multibits)) => {
                let var_typ = direct_variable_to_type(db, *dv, *multibits);
                if let Some(multibits) = multibits {
                    return multibits_to_type(db, *multibits);
                }
                var_typ
            }
            Type::StructElement(element) => Type::new_spec(db, element.spec(db)).normalize(db),
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
