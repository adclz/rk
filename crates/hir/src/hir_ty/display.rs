use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, Related};

use crate::{
    HasName, HirNodeInfo,
    hir_def::expressions::spec::{ElementarySpec, Spec, SpecKind},
    hir_ty::ty::{InferType, Type},
};

impl<'db> Type<'db> {
    pub fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            Self::Elementary(elem) => match elem {
                ElementarySpec::Bool => "BOOL",
                ElementarySpec::REDGEBool => "BOOL (RISING EDGE)",
                ElementarySpec::FEDGEBool => "BOOl (FALLING EDGE)",
                ElementarySpec::Byte => "BYTE",
                ElementarySpec::Word => "WORD",
                ElementarySpec::DWord => "DWORD",
                ElementarySpec::LWord => "LWORD",
                ElementarySpec::SInt => "SINT",
                ElementarySpec::USInt => "USINT",
                ElementarySpec::UInt => "UINT",
                ElementarySpec::Int => "INT",
                ElementarySpec::DInt => "DINT",
                ElementarySpec::UDInt => "UDINT",
                ElementarySpec::LInt => "LINT",
                ElementarySpec::ULInt => "ULINT",
                ElementarySpec::Real => "REAL",
                ElementarySpec::LReal => "LREAL",
                ElementarySpec::String => "STRING",
                ElementarySpec::WString => "WSTRING",
                ElementarySpec::Char => "CHAR",
                ElementarySpec::WChar => "WCHAR",
                ElementarySpec::Date => "DATE",
                ElementarySpec::LDate => "LDATE",
                ElementarySpec::DateAndTime => "DT",
                ElementarySpec::LDateTime => "LDT",
                ElementarySpec::Time => "TIME",
                ElementarySpec::LTime => "LTIME",
                ElementarySpec::Tod => "TOD",
                ElementarySpec::LTod => "LTOD",
            }
            .into(),
            Self::Function(f) => f.get_name_ident(db).text(db).to_string(),
            Self::FunctionBlock(fb) => fb.get_name_ident(db).text(db).to_string(),
            Self::MethodDecl(m) => m.get_name_ident(db).text(db).to_string(),
            Self::Class(c) => c.get_name_ident(db).text(db).to_string(),
            Self::Interface(i) => i.get_name_ident(db).text(db).to_string(),
            Self::DataType(typ) => match typ.spec(db).kind(db) {
                SpecKind::Target(e) => Type::new_spec(db, typ.spec(db)).type_name(db),
                _ => Type::new_spec(db, typ.spec(db)).type_name(db),
            },
            Self::Enum(_) => "ENUM".into(),
            Self::Struct(_) => "STRUCT".into(),
            Self::Never => "{unknown}".into(),
            Self::StructElement(st) => Type::new_spec(db, st.spec(db)).type_name(db),
            Self::Variable(var) => Type::new_spec(db, var.spec(db)).type_name(db),
            Self::Infer(infer) => match infer {
                InferType::Integer(i) => format!("(INT) {}", i.ident(db).text(db)),
                InferType::Float(f) => format!("(REAL) {}", f.text(db)),
            },
            _ => self.full_type_name(db),
        }
    }

    pub fn full_type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            Self::Array(array) => {
                let elem_type = Type::new_spec(db, array.of_type(db)).type_name(db);
                let dimensions: Vec<String> = array
                    .subranges(db)
                    .iter()
                    .map(|(lower, upper)| {
                        let lower = lower
                            .as_range(db)
                            .map(|n| n.to_string())
                            .unwrap_or_default();
                        let upper = upper
                            .as_range(db)
                            .map(|n| n.to_string())
                            .unwrap_or_default();
                        format!("[{lower}..{upper}]")
                    })
                    .collect();
                format!("ARRAY {} OF {}", dimensions.join(" "), elem_type)
            }
            Self::Enum(enum_) => format!("ENUM ({} members)", enum_.variants(db).len()),
            Self::SubRange(subrange) => {
                let lower = subrange
                    .lower(db)
                    .as_range(db)
                    .map(|n| n.to_string())
                    .unwrap_or_default();

                let upper = subrange
                    .upper(db)
                    .as_range(db)
                    .map(|n| n.to_string())
                    .unwrap_or_default();

                format!("SUBRANGE ({lower}..{upper})")
            }
            Self::Struct(ztruct) => format!("STRUCT ({} members)", ztruct.elements(db).len()),
            Self::RefTo(ref_to) => format!("REF TO {}", Type::new_spec(db, *ref_to).type_name(db),),
            _ => self.type_name(db),
        }
    }

    pub fn location(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        match self {
            Self::Variable(v) => match v.spec(db).kind(db) {
                SpecKind::Target(t) => {
                    Type::new_spec(db, v.spec(db)).location(db, diag);
                }
                _ => diag.with_related(Related::new(
                    format!(
                        "type is declared by variable '{}' here",
                        v.name(db).text(db)
                    ),
                    v.get_scope_id(db).file(db),
                    v.get_name_span(db),
                )),
            },
            Self::DataType(typ) => {
                diag.with_related(Related::new(
                    format!(
                        "type is defined by '{}' here",
                        typ.get_name_ident(db).text(db)
                    ),
                    typ.scope_id(db).file(db),
                    typ.spec(db).get_span(db),
                ));
            }
            Self::StructElement(elem) => {
                match elem.spec(db).kind(db) {
                    SpecKind::Target(t) => {
                        Type::new_spec(db, elem.spec(db)).location(db, diag);
                    }
                    _ => diag.with_related(Related::new(
                        format!(
                            "type is declared by struct element '{}' here",
                            elem.name(db).text(db)
                        ),
                        elem.get_scope_id(db).file(db),
                        elem.get_name_span(db),
                    )),
                }
            }
            _ => { /* No location info available */ }
        }
    }
}
