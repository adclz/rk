use auto_lsp::default::db::BaseDatabase;

use crate::{hir_def::expressions::spec::ElementarySpec, hir_ty::{array_resolver::resolve_range, ty2::{InferType, Type}}};

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
            }.into(),
            Self::Function(f) => format!("FUNCTION"),
            Self::FunctionBlock(fb) => format!("FUNCTION BLOCK"),
            Self::MethodDecl(m) => format!("METHOD"),
            Self::Class(c) => format!("CLASS"),
            Self::Interface(i) => format!("INTERFACE"),
            Self::Enum(_) => "ENUM".into(),
            Self::Struct(_) => "STRUCT".into(),
            Self::Never => "{unknown}".into(),
            Self::Infer(infer) => match infer {
                InferType::Integer(i) => format!("(INT) {}", i.ident(db).text(db)),
                InferType::Float(f) => format!("(REAL) {}", f.text(db)),
            }
            _ => self.full_type_name(db),
        }
    }

    pub fn full_type_name(&self, db: &'db dyn BaseDatabase) -> String {
        eprintln!("Getting full type name for {:?}", self);
        match self {
            Self::Array(array) => {
                let elem_type = array.of_type(db).type_name(db);
                let dimensions: Vec<String> = array
                    .subranges(db)
                    .iter()
                    .map(|(lower, upper)| {
                        let lower = resolve_range(db, *lower)
                            .map(|n| n.to_string())
                            .unwrap_or_default();
                        let upper = resolve_range(db, *upper)
                            .map(|n| n.to_string())
                            .unwrap_or_default();
                        format!("[{lower}..{upper}]")
                    })
                    .collect();
                format!("ARRAY {} OF {}", dimensions.join(" "), elem_type)
            }
            Self::Enum(enum_) => format!("ENUM ({} members)", enum_.variants(db).len()),
            Self::SubRange(subrange) => {
                let lower = resolve_range(db, subrange.lower(db))
                    .map(|n| n.to_string())
                    .unwrap_or_default();

                let upper = resolve_range(db, subrange.upper(db))
                    .map(|n| n.to_string())
                    .unwrap_or_default();

                format!("SUBRANGE ({lower}..{upper})")
            }
            Self::Struct(ztruct) => format!("STRUCT ({} members)", ztruct.elements(db).len()),
            Self::RefTo(ref_to) => format!("REF TO {}", ref_to.type_name(db)),
            _ => self.type_name(db),
        }
    }
}
